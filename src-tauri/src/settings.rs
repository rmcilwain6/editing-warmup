use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEFAULT_CHALLENGES: &[&str] = &[
    "Convert the photo to black and white. No color at all.",
    "Push the white balance drastically warmer or cooler than a neutral read.",
    "Use only the Effects panel: grain, dehaze, vignette. Leave every other panel at zero.",
    "Every slider you touch must land on exactly -100, 0, or +100. No in-between values.",
    "Crop to a square and recompose the shot entirely.",
    "Push exposure and contrast as far as you can while keeping the subject recognizable.",
    "Edit using only the Tone Curve panel. Leave the Basic panel untouched.",
    "Clip both tonal extremes on purpose: push the shadows to pure black and the highlights to a full blowout in the same edit.",
    "Desaturate everything except one color.",
    "Push the vignette as far as you can before it feels wrong.",
    "Use split toning to give the shadows and highlights two different color casts.",
    "Push sharpening, clarity, and dehaze all to their maximum, then back off by 10 percent.",
    "Try to recover as much detail as possible from the darkest shadows.",
    "Use only the HSL panel to change colors. Don't touch exposure or contrast.",
    "Add heavy grain, at least +50, and treat it as a deliberate stylistic choice rather than a flaw.",
    "Cut the saturation exactly in half.",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeDef {
    pub id: String,
    pub text: String,
    pub enabled: bool,
    #[serde(default)]
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub photos_per_session: usize,
    pub seconds_per_photo: u64,
    pub challenges: Vec<ChallengeDef>,
    #[serde(default)]
    pub last_archive_root: Option<String>,
    #[serde(default)]
    pub last_export_root: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            photos_per_session: 3,
            seconds_per_photo: 300,
            challenges: default_challenge_defs(),
            last_archive_root: None,
            last_export_root: None,
        }
    }
}

fn default_challenge_defs() -> Vec<ChallengeDef> {
    DEFAULT_CHALLENGES
        .iter()
        .enumerate()
        .map(|(i, text)| ChallengeDef {
            id: format!("builtin-{i}"),
            text: text.to_string(),
            enabled: true,
            custom: false,
        })
        .collect()
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("settings.json"))
}

fn merge_with_defaults(saved: Settings) -> Settings {
    let mut merged_challenges = default_challenge_defs();
    for def in merged_challenges.iter_mut() {
        if let Some(existing) = saved.challenges.iter().find(|c| c.id == def.id) {
            def.enabled = existing.enabled;
        }
    }
    for existing in saved.challenges.into_iter().filter(|c| c.custom) {
        merged_challenges.push(existing);
    }

    Settings {
        photos_per_session: saved.photos_per_session.clamp(1, 20),
        seconds_per_photo: saved.seconds_per_photo.clamp(5, 3600),
        challenges: merged_challenges,
        last_archive_root: saved.last_archive_root,
        last_export_root: saved.last_export_root,
    }
}

pub fn load_settings(app: &AppHandle) -> Settings {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(_) => return Settings::default(),
    };

    let loaded: Option<Settings> = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok());

    match loaded {
        Some(saved) => merge_with_defaults(saved),
        None => {
            let defaults = Settings::default();
            let _ = save_settings(app, &defaults);
            defaults
        }
    }
}

pub fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    fs::write(path, json).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_challenges_are_all_enabled_and_not_custom() {
        let defs = default_challenge_defs();
        assert!(!defs.is_empty());
        assert!(defs.iter().all(|c| c.enabled && !c.custom));
    }

    #[test]
    fn merge_with_defaults_preserves_disabled_builtin_flag() {
        let mut saved = Settings::default();
        saved.challenges[0].enabled = false;

        let merged = merge_with_defaults(saved);
        assert!(!merged.challenges[0].enabled);
        assert_eq!(merged.challenges.len(), default_challenge_defs().len());
    }

    #[test]
    fn merge_with_defaults_keeps_custom_challenges() {
        let mut saved = Settings::default();
        saved.challenges.push(ChallengeDef {
            id: "custom-1".to_string(),
            text: "My custom challenge".to_string(),
            enabled: true,
            custom: true,
        });

        let merged = merge_with_defaults(saved);
        assert!(merged
            .challenges
            .iter()
            .any(|c| c.id == "custom-1" && c.text == "My custom challenge"));
    }

    #[test]
    fn merge_with_defaults_drops_stale_builtin_ids_not_in_current_defaults() {
        let mut saved = Settings::default();
        saved.challenges.push(ChallengeDef {
            id: "builtin-old-removed".to_string(),
            text: "no longer a default".to_string(),
            enabled: true,
            custom: false,
        });

        let merged = merge_with_defaults(saved);
        assert!(!merged.challenges.iter().any(|c| c.id == "builtin-old-removed"));
    }

    #[test]
    fn merge_with_defaults_clamps_out_of_range_values() {
        let mut saved = Settings::default();
        saved.photos_per_session = 999;
        saved.seconds_per_photo = 1;

        let merged = merge_with_defaults(saved);
        assert_eq!(merged.photos_per_session, 20);
        assert_eq!(merged.seconds_per_photo, 5);
    }

    #[test]
    fn merge_with_defaults_preserves_last_used_folders() {
        let mut saved = Settings::default();
        saved.last_archive_root = Some("C:/archive".to_string());
        saved.last_export_root = Some("C:/exports".to_string());

        let merged = merge_with_defaults(saved);
        assert_eq!(merged.last_archive_root, Some("C:/archive".to_string()));
        assert_eq!(merged.last_export_root, Some("C:/exports".to_string()));
    }
}
