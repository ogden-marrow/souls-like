//! Co-op settings that exist in every game's Seamless Co-op settings file, mirrored across games.
//! The save-file extension is deliberately NOT in this list: it must stay per game.

use crate::{games::Game, ini, launch};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy)]
pub enum Kind {
    Text,
    Bool,
    Int(i64, i64),
}

pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    pub kind: Kind,
}

pub static SETTINGS: &[Setting] = &[
    Setting {
        key: "cooppassword",
        label: "Co-op password",
        help: "Friends need the same password to join your session",
        kind: Kind::Text,
    },
    Setting {
        key: "allow_invaders",
        label: "Allow invaders",
        help: "Other players can join your world uninvited",
        kind: Kind::Bool,
    },
    Setting {
        key: "death_debuffs",
        label: "Death debuffs",
        help: "Rot essence on death, cured at a bonfire",
        kind: Kind::Bool,
    },
    Setting {
        key: "overhead_player_display",
        label: "Overhead display",
        help: "0 off, 1 ping, 2 soul level, 3 deaths, 4 level and ping (meaning can differ per game)",
        kind: Kind::Int(0, 4),
    },
    Setting {
        key: "enemy_health_scaling",
        label: "Enemy health per player (%)",
        help: "Extra enemy health for each extra player",
        kind: Kind::Int(0, 1000),
    },
    Setting {
        key: "enemy_damage_scaling",
        label: "Enemy damage per player (%)",
        help: "Extra enemy damage for each extra player",
        kind: Kind::Int(0, 1000),
    },
    Setting {
        key: "enemy_posture_scaling",
        label: "Enemy posture per player (%)",
        help: "Extra enemy posture absorption for each extra player",
        kind: Kind::Int(0, 1000),
    },
    Setting {
        key: "boss_health_scaling",
        label: "Boss health per player (%)",
        help: "Extra boss health for each extra player",
        kind: Kind::Int(0, 2000),
    },
    Setting {
        key: "boss_damage_scaling",
        label: "Boss damage per player (%)",
        help: "Extra boss damage for each extra player",
        kind: Kind::Int(0, 1000),
    },
    Setting {
        key: "boss_posture_scaling",
        label: "Boss posture per player (%)",
        help: "Extra boss posture absorption for each extra player",
        kind: Kind::Int(0, 1000),
    },
];

/// Write every value in `values` into the settings file, but only for keys the file already has.
pub fn apply_to_file(path: &Path, values: &BTreeMap<String, String>) -> usize {
    let mut n = 0;
    for s in SETTINGS {
        if let Some(v) = values.get(s.key) {
            if ini::get(path, s.key).is_some() && ini::set(path, s.key, v).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// Mirror one setting into every installed game that has it.
pub fn apply_key(key: &str, value: &str, targets: &[(&Game, &Path)]) -> usize {
    let mut n = 0;
    for (g, dir) in targets {
        let p = launch::settings_path(g, dir);
        if ini::get(&p, key).is_some() && ini::set(&p, key, value).is_ok() {
            n += 1;
        }
    }
    n
}

/// Current value of a setting: the first game's file that has it.
pub fn read(key: &str, targets: &[(&Game, &Path)]) -> Option<String> {
    targets
        .iter()
        .find_map(|(g, dir)| ini::get(&launch::settings_path(g, dir), key))
}
