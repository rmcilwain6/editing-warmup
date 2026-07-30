use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEFAULT_CHALLENGES: &[&str] = &[
    "Convert the photo to black and white -- no color at all.",
    "Push the white balance drastically warmer or cooler than what looks \"correct.\"",
    "Use only sliders in the Effects panel (grain, dehaze, vignette) -- everything else stays at zero.",
    "Every slider you touch must land on exactly -100, 0, or +100 -- no in-between values.",
    "Make it look like it was shot on expired film.",
    "Crop to a square and recompose the shot entirely.",
    "Push exposure and contrast as far as you can while keeping the subject recognizable.",
    "Edit using only the Tone Curve panel -- leave the basic sliders untouched.",
    "Make the shadows pure black and blow out the highlights on purpose.",
    "Desaturate everything except one color.",
    "Push the vignette as far as you can before it feels wrong.",
    "Edit it as if it's the poster for a moody film noir.",
    "Use split toning to give shadows and highlights two different color casts.",
    "Push sharpening and clarity far beyond natural, then back off just slightly.",
    "Try to recover as much detail as possible from the darkest shadows.",
    "Make it look overexposed and dreamy -- blow out the highlights intentionally.",
    "Only use the HSL panel to change colors -- don't touch exposure or contrast.",
    "Edit it for a black-and-white newspaper print.",
    "Add heavy grain and treat it as a deliberate stylistic choice, not a flaw.",
    "Edit for a cold, blue-toned \"winter morning\" mood, regardless of when it was shot.",
    "Make a fairly plain photo look like a movie still.",
    "Cut the saturation in half and see what the photo still says without color intensity.",
    "Push clarity and dehaze to their max -- embrace the harsh, gritty look.",
    "Edit as if inverting tonal expectations, like a print negative.",
    "Give yourself 60 seconds and don't second-guess any slider.",
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            photos_per_session: 3,
            seconds_per_photo: 300,
            challenges: default_challenge_defs(),
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
