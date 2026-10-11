//! Persistent settings (per-user JSON).

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Vanilla,
    #[default]
    Seamless,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct GameState {
    pub mode: Mode,
    pub mods: BTreeMap<String, bool>,
    /// Optional manual override of the folder holding the save files (macOS bottles, synced folders...).
    pub save_dir: String,
    /// Optional manual override of the game's install folder.
    pub game_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub last: String,
    /// "system" | "light" | "dark"
    pub theme: String,
    pub games: BTreeMap<String, GameState>,
    /// Mirror the shared co-op settings across every game's Seamless Co-op settings file.
    pub mirror: bool,
    /// Values the user set on the shared-settings page (applied to each game at launch while mirroring).
    pub shared: BTreeMap<String, String>,
    /// Shared settings the user opted out of mirroring.
    pub unsynced: BTreeSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            last: String::new(),
            theme: "system".into(),
            games: BTreeMap::new(),
            mirror: true,
            shared: BTreeMap::new(),
            unsynced: BTreeSet::new(),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("souls-like")
        .join("config.json")
}

pub fn log_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("souls-like")
        .join("logs")
}

impl Config {
    pub fn load() -> Config {
        fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = config_path();
        if let Some(d) = p.parent() {
            let _ = fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, s);
        }
    }

    pub fn game(&mut self, key: &str) -> &mut GameState {
        self.games.entry(key.to_string()).or_default()
    }
}
