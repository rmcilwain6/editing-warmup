#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;

use chrono::Local;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::Serialize;
use settings::Settings;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const ATTEMPTS_PER_PHOTO: usize = 25;

#[derive(Debug, Clone, Serialize)]
struct StepPayload {
    step_index: usize,
    total_steps: usize,
    raw_path: String,
    expected_jpg: String,
    seconds_remaining: u64,
    challenge: String,
}

#[derive(Debug, Clone, Serialize)]
struct SessionFinishedPayload {
    export_dir: String,
    exports: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Debug, Clone)]
struct StepRecord {
    raw_path: PathBuf,
    challenge: String,
    expected_jpg: PathBuf,
    outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct HistoryEntry {
    timestamp: String,
    export_dir: String,
    raw_path: String,
    challenge: String,
    expected_jpg: String,
    outcome: String,
}

#[derive(Debug, Clone)]
struct SessionState {
    export_dir: PathBuf,
    archive_root: PathBuf,
    picks: Vec<PathBuf>,
    current_idx: usize,
    session_id: usize,
    step_generation: usize,
    step_started_at: Instant,
    expected_jpg: PathBuf,
    seconds_per_photo: u64,
    records: Vec<StepRecord>,
    started_at_label: String,
}

#[derive(Default)]
struct AppState {
    session: Option<SessionState>,
    watcher: Option<RecommendedWatcher>,
}

#[derive(Clone)]
struct SharedState {
    inner: Arc<Mutex<AppState>>,
    current_session: Arc<Mutex<usize>>,
    settings: Arc<Mutex<Settings>>,
}

#[tauri::command]
fn get_settings(state: tauri::State<SharedState>) -> Result<Settings, String> {
    let guard = state.settings.lock().map_err(|_| "Lock error")?;
    Ok(guard.clone())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<SharedState>,
    settings: Settings,
) -> Result<(), String> {
    let clamped = Settings {
        photos_per_session: settings.photos_per_session.clamp(1, 20),
        seconds_per_photo: settings.seconds_per_photo.clamp(5, 3600),
        challenges: settings.challenges,
        last_archive_root: settings.last_archive_root,
        last_export_root: settings.last_export_root,
    };
    settings::save_settings(&app, &clamped)?;
    let mut guard = state.settings.lock().map_err(|_| "Lock error")?;
    *guard = clamped;
    Ok(())
}

fn pick_challenge(settings: &Settings) -> String {
    let mut rng = rand::thread_rng();
    let enabled: Vec<&settings::ChallengeDef> =
        settings.challenges.iter().filter(|c| c.enabled).collect();
    match enabled.choose(&mut rng) {
        Some(challenge) => challenge.text.clone(),
        None => "Edit the photo (no challenges enabled -- check Settings).".to_string(),
    }
}

#[tauri::command]
fn start_session(
    app: AppHandle,
    state: tauri::State<SharedState>,
    archive_root: String,
    export_root: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let session_id = {
        let mut guard = state.current_session.lock().map_err(|_| "Lock error")?;
        *guard += 1;
        *guard
    };

    let archive_root_path = Path::new(&archive_root);
    if !archive_root_path.is_dir() {
        return Err(format!("Archive folder not found: {}", archive_root_path.display()));
    }

    let (photos_per_session, seconds_per_photo) = {
        let mut guard = state.settings.lock().map_err(|_| "Lock error")?;
        guard.last_archive_root = Some(archive_root.clone());
        guard.last_export_root = Some(export_root.clone());
        settings::save_settings(&app, &guard)?;
        (guard.photos_per_session, guard.seconds_per_photo)
    };

    let started_at_label = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let export_dir =
        create_session_dir(&export_root, &started_at_label).map_err(|err| err.to_string())?;
    let picks = pick_random_files(archive_root_path, photos_per_session)
        .map_err(|err| err.to_string())?;

    if picks.len() < photos_per_session {
        app.emit(
            "session_error",
            ErrorPayload {
                message: "Unable to locate enough .CR2 files with random walk.".to_string(),
            },
        )
        .ok();
        return Ok(());
    }

    {
        let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
        guard.session = Some(SessionState {
            export_dir: export_dir.clone(),
            archive_root: archive_root_path.to_path_buf(),
            picks: picks.clone(),
            current_idx: 0,
            session_id,
            step_generation: 0,
            step_started_at: Instant::now(),
            expected_jpg: PathBuf::new(),
            seconds_per_photo,
            records: Vec::new(),
            started_at_label,
        });
    }

    start_watcher(app.clone(), state.clone(), export_dir.clone(), session_id)?;
    start_step(app, state, session_id)?;

    Ok(())
}

#[tauri::command]
fn manual_next(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let state = state.inner().clone();
    advance_step(app, state, false)
}

#[tauri::command]
fn skip_step(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let state = state.inner().clone();
    advance_step(app, state, true)
}

#[tauri::command]
fn keep_working(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let state = state.inner().clone();
    let guard = state.inner.lock().map_err(|_| "Lock error")?;
    if let Some(session) = &guard.session {
        app.emit("step_resumed", session.current_idx).ok();
    }
    Ok(())
}

#[tauri::command]
fn reject_current(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let state = state.inner().clone();
    let session_id = {
        let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
        let session = match guard.session.as_mut() {
            Some(session) => session,
            None => return Ok(()),
        };

        if session.current_idx == 0 {
            return Err("No active photo to reject.".to_string());
        }

        let slot = session.current_idx - 1;
        let exclude: HashSet<PathBuf> = session.picks.iter().cloned().collect();
        let replacement = pick_replacement(&session.archive_root, &exclude)
            .ok_or_else(|| "No replacement photo available in the archive.".to_string())?;

        session.picks[slot] = replacement;
        session.current_idx = slot;
        if session.records.len() > slot {
            session.records.truncate(slot);
        }
        session.session_id
    };

    start_step(app, state, session_id)
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    open::that(path).map_err(|err| err.to_string())
}

#[tauri::command]
fn open_export_folder(state: tauri::State<SharedState>) -> Result<(), String> {
    let state = state.inner().clone();
    let guard = state.inner.lock().map_err(|_| "Lock error")?;
    if let Some(session) = &guard.session {
        open::that(&session.export_dir).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn advance_step(app: AppHandle, state: SharedState, skipped: bool) -> Result<(), String> {
    let session_id = {
        let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
        let session = guard.session.as_mut();
        let session_id = session.as_ref().map(|s| s.session_id).unwrap_or(0);
        if let Some(session) = session {
            finalize_current_record(session, if skipped { Some("skipped") } else { None });
        }
        session_id
    };
    start_step(app, state, session_id)
}

fn finalize_current_record(session: &mut SessionState, forced_outcome: Option<&str>) {
    if let Some(record) = session.records.last_mut() {
        if record.outcome.is_none() {
            record.outcome = Some(match forced_outcome {
                Some(outcome) => outcome.to_string(),
                None => {
                    if record.expected_jpg.exists() {
                        "exported".to_string()
                    } else {
                        "abandoned".to_string()
                    }
                }
            });
        }
    }
}

fn start_step(app: AppHandle, state: SharedState, session_id: usize) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
    let session = match guard.session.as_mut() {
        Some(session) => session,
        None => return Ok(()),
    };

    if session.session_id != session_id {
        return Ok(());
    }

    if session.current_idx >= session.picks.len() {
        finalize_current_record(session, None);
        let exports = list_exports(&session.export_dir);
        write_history(&app, session);
        app.emit(
            "session_finished",
            SessionFinishedPayload {
                export_dir: session.export_dir.display().to_string(),
                exports,
            },
        )
        .ok();
        return Ok(());
    }

    let raw_path = session.picks[session.current_idx].clone();
    let expected_jpg = expected_export_path(&session.export_dir, &raw_path);
    session.expected_jpg = expected_jpg.clone();
    session.step_started_at = Instant::now();
    session.step_generation += 1;
    let generation = session.step_generation;
    let seconds_per_photo = session.seconds_per_photo;

    let challenge = {
        let settings_guard = state.settings.lock().map_err(|_| "Lock error")?;
        pick_challenge(&settings_guard)
    };

    session.records.push(StepRecord {
        raw_path: raw_path.clone(),
        challenge: challenge.clone(),
        expected_jpg: expected_jpg.clone(),
        outcome: None,
    });

    let payload = StepPayload {
        step_index: session.current_idx + 1,
        total_steps: session.picks.len(),
        raw_path: raw_path.display().to_string(),
        expected_jpg: expected_jpg.file_name().unwrap_or_default().to_string_lossy().to_string(),
        seconds_remaining: seconds_per_photo,
        challenge,
    };

    app.emit("step_started", payload).ok();
    open::that(&raw_path).map_err(|err| err.to_string())?;
    start_timer(
        app.clone(),
        state.clone(),
        session.session_id,
        generation,
        seconds_per_photo,
    );

    session.current_idx += 1;
    Ok(())
}

fn start_timer(app: AppHandle, state: SharedState, session_id: usize, generation: usize, seconds_per_photo: u64) {
    thread::spawn(move || {
        for remaining in (0..=seconds_per_photo).rev() {
            {
                let guard = state.inner.lock();
                if let Ok(guard) = guard {
                    let session = match &guard.session {
                        Some(session) => session,
                        None => return,
                    };
                    if session.session_id != session_id || session.step_generation != generation {
                        return;
                    }
                }
            }

            app.emit("timer_tick", remaining).ok();
            if remaining == 0 {
                app.emit("time_expired", generation).ok();
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn start_watcher(app: AppHandle, state: SharedState, export_dir: PathBuf, session_id: usize) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        tx.send(res).ok();
    })
    .map_err(|err| err.to_string())?;

    watcher
        .watch(&export_dir, RecursiveMode::NonRecursive)
        .map_err(|err| err.to_string())?;

    {
        let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
        guard.watcher = Some(watcher);
    }

    thread::spawn(move || {
        while let Ok(res) = rx.recv() {
            if res.is_err() {
                continue;
            }

            let (expected, current_idx) = {
                let guard = state.inner.lock();
                if let Ok(guard) = guard {
                    let session = match &guard.session {
                        Some(session) => session,
                        None => continue,
                    };
                    if session.session_id != session_id {
                        continue;
                    }
                    (session.expected_jpg.clone(), session.current_idx)
                } else {
                    continue;
                }
            };

            if expected.as_os_str().is_empty() {
                continue;
            }

            if expected.exists() && is_stable(&expected) {
                app.emit("export_detected", current_idx).ok();
                let app_clone = app.clone();
                let state_clone = state.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(1000));
                    if let Ok(mut guard) = state_clone.inner.lock() {
                        if let Some(session) = guard.session.as_mut() {
                            if session.session_id == session_id {
                                finalize_current_record(session, Some("exported"));
                            }
                        }
                    }
                    let _ = start_step(app_clone, state_clone, session_id);
                });
            }
        }
    });

    Ok(())
}

fn is_stable(path: &Path) -> bool {
    let size_one = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    thread::sleep(Duration::from_millis(1000));
    let size_two = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    size_one > 0 && size_one == size_two
}

fn expected_export_path(export_dir: &Path, raw_path: &Path) -> PathBuf {
    let stem = raw_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    export_dir.join(format!("{stem}.jpg"))
}

fn create_session_dir(export_root: &str, timestamp: &str) -> Result<PathBuf, std::io::Error> {
    let session_dir = Path::new(export_root).join("Warmups").join(timestamp);
    fs::create_dir_all(&session_dir)?;
    Ok(session_dir)
}

fn pick_random_files(root: &Path, count: usize) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut picks = Vec::new();
    let mut seen = HashSet::new();
    let mut rng = rand::thread_rng();

    while picks.len() < count {
        let mut found = None;
        for _ in 0..ATTEMPTS_PER_PHOTO {
            if let Some(candidate) = random_walk_pick(root, &mut rng) {
                if seen.insert(candidate.clone()) {
                    found = Some(candidate);
                    break;
                }
            }
        }

        if let Some(candidate) = found {
            picks.push(candidate);
        } else {
            break;
        }
    }

    Ok(picks)
}

fn pick_replacement(root: &Path, exclude: &HashSet<PathBuf>) -> Option<PathBuf> {
    let mut rng = rand::thread_rng();
    for _ in 0..ATTEMPTS_PER_PHOTO {
        if let Some(candidate) = random_walk_pick(root, &mut rng) {
            if !exclude.contains(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn random_walk_pick(root: &Path, rng: &mut rand::rngs::ThreadRng) -> Option<PathBuf> {
    let depth = rng.gen_range(2..=8);
    let mut current = root.to_path_buf();

    for _ in 0..depth {
        let entries = fs::read_dir(&current).ok()?;
        let mut dirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
        if dirs.is_empty() {
            break;
        }
        current = dirs.choose(rng)?.clone();
    }

    let mut pool = Vec::new();
    collect_cr2_files(&current, &mut pool);
    if let Ok(entries) = fs::read_dir(&current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_cr2_files(&path, &mut pool);
            }
        }
    }

    pool.choose(rng).cloned()
}

fn collect_cr2_files(dir: &Path, pool: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_cr2(&path) {
                pool.push(path);
            }
        }
    }
}

fn is_cr2(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cr2"))
        .unwrap_or(false)
}

fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("history.jsonl"))
}

fn history_entries_from_session(session: &SessionState) -> Vec<HistoryEntry> {
    session
        .records
        .iter()
        .map(|record| HistoryEntry {
            timestamp: session.started_at_label.clone(),
            export_dir: session.export_dir.display().to_string(),
            raw_path: record.raw_path.display().to_string(),
            challenge: record.challenge.clone(),
            expected_jpg: record
                .expected_jpg
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            outcome: record
                .outcome
                .clone()
                .unwrap_or_else(|| "abandoned".to_string()),
        })
        .collect()
}

fn append_history_entries(path: &Path, entries: &[HistoryEntry]) {
    let mut lines = String::new();
    for entry in entries {
        if let Ok(json) = serde_json::to_string(entry) {
            lines.push_str(&json);
            lines.push('\n');
        }
    }

    if lines.is_empty() {
        return;
    }

    use std::io::Write;
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(lines.as_bytes());
    }
}

fn parse_history(content: &str) -> Vec<HistoryEntry> {
    let mut entries: Vec<HistoryEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    entries.reverse();
    entries
}

fn write_history(app: &AppHandle, session: &SessionState) {
    let path = match history_path(app) {
        Ok(path) => path,
        Err(_) => return,
    };
    let entries = history_entries_from_session(session);
    append_history_entries(&path, &entries);
}

#[tauri::command]
fn get_history(app: AppHandle) -> Result<Vec<HistoryEntry>, String> {
    let path = history_path(&app)?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return Ok(Vec::new()),
    };
    Ok(parse_history(&content))
}

fn list_exports(export_dir: &Path) -> Vec<String> {
    let mut exports = Vec::new();
    if let Ok(entries) = fs::read_dir(export_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                    if ext.eq_ignore_ascii_case("jpg") {
                        exports.push(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    exports.sort();
    exports
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn blank_session(export_dir: PathBuf, archive_root: PathBuf) -> SessionState {
        SessionState {
            export_dir,
            archive_root,
            picks: Vec::new(),
            current_idx: 0,
            session_id: 1,
            step_generation: 0,
            step_started_at: Instant::now(),
            expected_jpg: PathBuf::new(),
            seconds_per_photo: 60,
            records: Vec::new(),
            started_at_label: "2026-01-01_12-00-00".to_string(),
        }
    }

    #[test]
    fn is_cr2_matches_case_insensitively() {
        assert!(is_cr2(Path::new("photo.CR2")));
        assert!(is_cr2(Path::new("photo.cr2")));
        assert!(!is_cr2(Path::new("photo.jpg")));
        assert!(!is_cr2(Path::new("photo")));
    }

    #[test]
    fn expected_export_path_uses_raw_stem_with_jpg_extension() {
        let export_dir = Path::new("C:/exports");
        let raw_path = Path::new("C:/archive/IMG_1234.CR2");
        let expected = expected_export_path(export_dir, raw_path);
        assert_eq!(expected, Path::new("C:/exports/IMG_1234.jpg"));
    }

    #[test]
    fn pick_challenge_returns_only_enabled_text() {
        let settings = Settings {
            photos_per_session: 3,
            seconds_per_photo: 60,
            challenges: vec![
                settings::ChallengeDef {
                    id: "a".into(),
                    text: "only enabled".into(),
                    enabled: true,
                    custom: false,
                },
                settings::ChallengeDef {
                    id: "b".into(),
                    text: "disabled".into(),
                    enabled: false,
                    custom: false,
                },
            ],
            last_archive_root: None,
            last_export_root: None,
        };
        assert_eq!(pick_challenge(&settings), "only enabled");
    }

    #[test]
    fn pick_challenge_falls_back_when_none_enabled() {
        let settings = Settings {
            photos_per_session: 3,
            seconds_per_photo: 60,
            challenges: vec![settings::ChallengeDef {
                id: "a".into(),
                text: "disabled".into(),
                enabled: false,
                custom: false,
            }],
            last_archive_root: None,
            last_export_root: None,
        };
        assert!(pick_challenge(&settings).contains("no challenges enabled"));
    }

    #[test]
    fn create_session_dir_creates_nested_timestamped_folder() {
        let temp = tempfile::tempdir().unwrap();
        let export_root = temp.path().to_str().unwrap();
        let session_dir = create_session_dir(export_root, "2026-01-01_09-00-00").unwrap();
        assert!(session_dir.is_dir());
        assert!(session_dir.ends_with("Warmups/2026-01-01_09-00-00")
            || session_dir.ends_with("Warmups\\2026-01-01_09-00-00"));
    }

    #[test]
    fn list_exports_only_returns_sorted_jpgs() {
        let temp = tempfile::tempdir().unwrap();
        File::create(temp.path().join("b.jpg")).unwrap();
        File::create(temp.path().join("a.JPG")).unwrap();
        File::create(temp.path().join("notes.txt")).unwrap();
        let exports = list_exports(temp.path());
        assert_eq!(exports, vec!["a.JPG".to_string(), "b.jpg".to_string()]);
    }

    #[test]
    fn pick_random_files_returns_unique_cr2_files() {
        let temp = tempfile::tempdir().unwrap();
        let nested = temp.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        for i in 0..5 {
            File::create(nested.join(format!("IMG_{i}.CR2"))).unwrap();
        }
        File::create(nested.join("ignore.txt")).unwrap();

        let picks = pick_random_files(temp.path(), 3).unwrap();
        assert_eq!(picks.len(), 3);

        let unique: HashSet<_> = picks.iter().collect();
        assert_eq!(unique.len(), 3);

        for pick in &picks {
            assert!(is_cr2(pick));
        }
    }

    #[test]
    fn finalize_current_record_uses_forced_outcome() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = blank_session(temp.path().to_path_buf(), temp.path().to_path_buf());
        session.records.push(StepRecord {
            raw_path: temp.path().join("raw.CR2"),
            challenge: "test".to_string(),
            expected_jpg: temp.path().join("missing.jpg"),
            outcome: None,
        });

        finalize_current_record(&mut session, Some("skipped"));
        assert_eq!(session.records.last().unwrap().outcome.as_deref(), Some("skipped"));
    }

    #[test]
    fn finalize_current_record_auto_detects_exported_when_file_exists() {
        let temp = tempfile::tempdir().unwrap();
        let jpg_path = temp.path().join("out.jpg");
        File::create(&jpg_path).unwrap();

        let mut session = blank_session(temp.path().to_path_buf(), temp.path().to_path_buf());
        session.records.push(StepRecord {
            raw_path: temp.path().join("raw.CR2"),
            challenge: "test".to_string(),
            expected_jpg: jpg_path,
            outcome: None,
        });

        finalize_current_record(&mut session, None);
        assert_eq!(session.records.last().unwrap().outcome.as_deref(), Some("exported"));
    }

    #[test]
    fn finalize_current_record_auto_detects_abandoned_when_no_export() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = blank_session(temp.path().to_path_buf(), temp.path().to_path_buf());
        session.records.push(StepRecord {
            raw_path: temp.path().join("raw.CR2"),
            challenge: "test".to_string(),
            expected_jpg: temp.path().join("never-exported.jpg"),
            outcome: None,
        });

        finalize_current_record(&mut session, None);
        assert_eq!(session.records.last().unwrap().outcome.as_deref(), Some("abandoned"));
    }

    #[test]
    fn finalize_current_record_does_not_overwrite_existing_outcome() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = blank_session(temp.path().to_path_buf(), temp.path().to_path_buf());
        session.records.push(StepRecord {
            raw_path: temp.path().join("raw.CR2"),
            challenge: "test".to_string(),
            expected_jpg: temp.path().join("never-exported.jpg"),
            outcome: Some("skipped".to_string()),
        });

        finalize_current_record(&mut session, Some("exported"));
        assert_eq!(session.records.last().unwrap().outcome.as_deref(), Some("skipped"));
    }

    #[test]
    fn history_entries_from_session_maps_records_with_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let mut session = blank_session(
            temp.path().join("export"),
            temp.path().join("archive"),
        );
        session.records.push(StepRecord {
            raw_path: temp.path().join("IMG_1.CR2"),
            challenge: "Go black and white".to_string(),
            expected_jpg: temp.path().join("export").join("IMG_1.jpg"),
            outcome: None,
        });

        let entries = history_entries_from_session(&session);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timestamp, "2026-01-01_12-00-00");
        assert_eq!(entries[0].challenge, "Go black and white");
        assert_eq!(entries[0].expected_jpg, "IMG_1.jpg");
        assert_eq!(entries[0].outcome, "abandoned");
    }

    #[test]
    fn append_and_parse_history_round_trips_in_reverse_order() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.jsonl");

        let first = HistoryEntry {
            timestamp: "2026-01-01_09-00-00".to_string(),
            export_dir: "C:/exports/1".to_string(),
            raw_path: "C:/archive/IMG_1.CR2".to_string(),
            challenge: "first".to_string(),
            expected_jpg: "IMG_1.jpg".to_string(),
            outcome: "exported".to_string(),
        };
        let second = HistoryEntry {
            timestamp: "2026-01-02_09-00-00".to_string(),
            export_dir: "C:/exports/2".to_string(),
            raw_path: "C:/archive/IMG_2.CR2".to_string(),
            challenge: "second".to_string(),
            expected_jpg: "IMG_2.jpg".to_string(),
            outcome: "skipped".to_string(),
        };

        append_history_entries(&path, &[first.clone()]);
        append_history_entries(&path, &[second.clone()]);

        let content = fs::read_to_string(&path).unwrap();
        let parsed = parse_history(&content);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].challenge, "second");
        assert_eq!(parsed[1].challenge, "first");
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(SharedState {
            inner: Arc::new(Mutex::new(AppState::default())),
            current_session: Arc::new(Mutex::new(0)),
            settings: Arc::new(Mutex::new(Settings::default())),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let loaded = settings::load_settings(&handle);
            let state: tauri::State<SharedState> = app.state();
            *state.settings.lock().map_err(|_| "Lock error")? = loaded;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            start_session,
            manual_next,
            skip_step,
            keep_working,
            open_export_folder,
            reject_current,
            get_history,
            open_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
