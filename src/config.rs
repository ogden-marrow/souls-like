use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
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
    pub save_dir: String,
    pub game_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub last: String,
    pub theme: String,
    pub games: BTreeMap<String, GameState>,
    pub mirror: bool,
    pub shared: BTreeMap<String, String>,
    pub unsynced: BTreeSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { last: String::new(), theme: "system".into(), games: BTreeMap::new(), mirror: true, shared: BTreeMap::new(), unsynced: BTreeSet::new() }
    }
}

fn app_dir(base: Option<PathBuf>) -> PathBuf {
    base.unwrap_or_default().join("souls-like")
}

pub fn log_dir() -> PathBuf {
    app_dir(dirs::data_dir()).join("logs")
}

impl Config {
    fn path() -> PathBuf {
        app_dir(dirs::config_dir()).join("config.json")
    }

    pub fn load() -> Config {
        fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        let _ = fs::create_dir_all(p.parent().unwrap_or(&p));
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, s);
        }
    }

    pub fn game(&mut self, key: &str) -> &mut GameState {
        self.games.entry(key.to_string()).or_default()
    }
}
