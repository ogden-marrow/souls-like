use crate::{games::Game, ini, launch};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy)]
pub enum Kind {
    Text,
    Bool,
    Int(i64, i64),
}

pub type Setting = (&'static str, &'static str, &'static str, Kind);

pub static SETTINGS: [Setting; 10] = [
    ("cooppassword", "Co-op password", "Friends need the same password to join your session", Kind::Text),
    ("allow_invaders", "Allow invaders", "Other players can join your world uninvited", Kind::Bool),
    ("death_debuffs", "Death debuffs", "Rot essence on death, cured at a bonfire", Kind::Bool),
    ("overhead_player_display", "Overhead display", "0 off, 1 ping, 2 soul level, 3 deaths, 4 level and ping (meaning can differ per game)", Kind::Int(0, 4)),
    ("enemy_health_scaling", "Enemy health per player (%)", "Extra enemy health for each extra player", Kind::Int(0, 1000)),
    ("enemy_damage_scaling", "Enemy damage per player (%)", "Extra enemy damage for each extra player", Kind::Int(0, 1000)),
    ("enemy_posture_scaling", "Enemy posture per player (%)", "Extra enemy posture absorption for each extra player", Kind::Int(0, 1000)),
    ("boss_health_scaling", "Boss health per player (%)", "Extra boss health for each extra player", Kind::Int(0, 2000)),
    ("boss_damage_scaling", "Boss damage per player (%)", "Extra boss damage for each extra player", Kind::Int(0, 1000)),
    ("boss_posture_scaling", "Boss posture per player (%)", "Extra boss posture absorption for each extra player", Kind::Int(0, 1000)),
];

pub fn apply_to_file(path: &Path, values: &BTreeMap<String, String>) {
    for (key, ..) in &SETTINGS {
        if let Some(v) = values.get(*key).filter(|_| ini::get(path, key).is_some()) {
            let _ = ini::set(path, key, v);
        }
    }
}

pub fn apply_key(key: &str, value: &str, targets: &[(&Game, &Path)]) -> usize {
    targets
        .iter()
        .filter(|(g, dir)| {
            let p = launch::settings_path(g, dir);
            ini::get(&p, key).is_some() && ini::set(&p, key, value).is_ok()
        })
        .count()
}

pub fn read(key: &str, targets: &[(&Game, &Path)]) -> Option<String> {
    targets.iter().find_map(|(g, dir)| ini::get(&launch::settings_path(g, dir), key))
}
