#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Local;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

// TODO: Update ARCHIVE_ROOT to your RAW archive folder.
const ARCHIVE_ROOT: &str = r"X:\TODO\YOUR\ARCHIVE\ROOT";
// TODO: Update EXPORT_ROOT to your desired export folder root.
const EXPORT_ROOT: &str = r"C:\TODO\YOUR\EXPORT\ROOT";
const PHOTOS_PER_SESSION: usize = 3;
const SECONDS_PER_PHOTO: u64 = 300;
const ATTEMPTS_PER_PHOTO: usize = 25;

#[derive(Debug, Clone, Serialize)]
struct StepPayload {
    step_index: usize,
    total_steps: usize,
    raw_path: String,
    expected_jpg: String,
    seconds_remaining: u64,
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
struct SessionState {
    export_dir: PathBuf,
    picks: Vec<PathBuf>,
    current_idx: usize,
    session_id: usize,
    step_started_at: Instant,
    expected_jpg: PathBuf,
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
}

#[tauri::command]
fn get_config() -> (String, String) {
    (ARCHIVE_ROOT.to_string(), EXPORT_ROOT.to_string())
}

#[tauri::command]
fn start_session(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let session_id = {
        let mut guard = state.current_session.lock().map_err(|_| "Lock error")?;
        *guard += 1;
        *guard
    };

    let export_dir = create_session_dir().map_err(|err| err.to_string())?;
    let picks = pick_random_files(Path::new(ARCHIVE_ROOT), PHOTOS_PER_SESSION)
        .map_err(|err| err.to_string())?;

    if picks.len() < PHOTOS_PER_SESSION {
        app.emit_all(
            "session_error",
            ErrorPayload {
                message: "Unable to locate enough .CR2 files with random walk.".to_string(),
            },
        )
        .ok();
        return Ok(());
    }

    let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
    guard.session = Some(SessionState {
        export_dir: export_dir.clone(),
        picks: picks.clone(),
        current_idx: 0,
        session_id,
        step_started_at: Instant::now(),
        expected_jpg: PathBuf::new(),
    });

    start_watcher(app.clone(), state.clone(), export_dir.clone(), session_id)?;
    start_step(app, state, session_id)?;

    Ok(())
}

#[tauri::command]
fn manual_next(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    advance_step(app, state, false)
}

#[tauri::command]
fn skip_step(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    advance_step(app, state, true)
}

#[tauri::command]
fn keep_working(app: AppHandle, state: tauri::State<SharedState>) -> Result<(), String> {
    let guard = state.inner.lock().map_err(|_| "Lock error")?;
    if let Some(session) = &guard.session {
        app.emit_all("step_resumed", session.current_idx).ok();
    }
    Ok(())
}

#[tauri::command]
fn open_export_folder(state: tauri::State<SharedState>) -> Result<(), String> {
    let guard = state.inner.lock().map_err(|_| "Lock error")?;
    if let Some(session) = &guard.session {
        open::that(&session.export_dir).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn advance_step(app: AppHandle, state: tauri::State<SharedState>, _skipped: bool) -> Result<(), String> {
    let session_id = {
        let guard = state.inner.lock().map_err(|_| "Lock error")?;
        guard.session.as_ref().map(|s| s.session_id).unwrap_or(0)
    };
    start_step(app, state, session_id)
}

fn start_step(app: AppHandle, state: tauri::State<SharedState>, session_id: usize) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|_| "Lock error")?;
    let session = match guard.session.as_mut() {
        Some(session) => session,
        None => return Ok(()),
    };

    if session.session_id != session_id {
        return Ok(());
    }

    if session.current_idx >= session.picks.len() {
        let exports = list_exports(&session.export_dir);
        app.emit_all(
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

    let payload = StepPayload {
        step_index: session.current_idx + 1,
        total_steps: session.picks.len(),
        raw_path: raw_path.display().to_string(),
        expected_jpg: expected_jpg.file_name().unwrap_or_default().to_string_lossy().to_string(),
        seconds_remaining: SECONDS_PER_PHOTO,
    };

    app.emit_all("step_started", payload).ok();
    open::that(&raw_path).map_err(|err| err.to_string())?;
    start_timer(app.clone(), state.clone(), session.session_id, session.current_idx);

    session.current_idx += 1;
    Ok(())
}

fn start_timer(app: AppHandle, state: tauri::State<SharedState>, session_id: usize, step_idx: usize) {
    thread::spawn(move || {
        for remaining in (0..=SECONDS_PER_PHOTO).rev() {
            {
                let guard = state.inner.lock();
                if let Ok(guard) = guard {
                    let session = match &guard.session {
                        Some(session) => session,
                        None => return,
                    };
                    if session.session_id != session_id || session.current_idx != step_idx + 1 {
                        return;
                    }
                }
            }

            app.emit_all("timer_tick", remaining).ok();
            if remaining == 0 {
                app.emit_all("time_expired", step_idx + 1).ok();
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn start_watcher(
    app: AppHandle,
    state: tauri::State<SharedState>,
    export_dir: PathBuf,
    session_id: usize,
) -> Result<(), String> {
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
                app.emit_all("export_detected", current_idx).ok();
                let app_clone = app.clone();
                let state_clone = state.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(1000));
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

fn create_session_dir() -> Result<PathBuf, std::io::Error> {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let session_dir = Path::new(EXPORT_ROOT)
        .join("Warmups")
        .join(timestamp.to_string());
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

fn main() {
    tauri::Builder::default()
        .manage(SharedState {
            inner: Arc::new(Mutex::new(AppState::default())),
            current_session: Arc::new(Mutex::new(0)),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            start_session,
            manual_next,
            skip_step,
            keep_working,
            open_export_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
