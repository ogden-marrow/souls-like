//! Prepare a game for the chosen mode and start it. Safety rules live here:
//! vanilla = every mod loader off; modded = never the normal `.sl2` save; backup before every launch.

use crate::{
    config::{GameState, Mode},
    games::Game,
    ini, platform, saves, shared,
};
use std::{fs, path::Path};

fn settings_ini(game: &Game, dir: &Path) -> std::path::PathBuf {
    dir.join("SeamlessCoop")
        .join(format!("{}_settings.ini", game.sc))
}

pub fn has_seamless(game: &Game, dir: &Path) -> bool {
    dir.join(format!("{}_launcher.exe", game.sc)).exists() && settings_ini(game, dir).exists()
}

pub fn settings_path(game: &Game, dir: &Path) -> std::path::PathBuf {
    settings_ini(game, dir)
}

/// Mod loader (dinput8.dll) on/off. Off = renamed, so the game really is vanilla.
fn set_dll(dir: &Path, on: bool) -> Result<(), String> {
    let live = dir.join("dinput8.dll");
    let off = dir.join("dinput8.dll.off");
    let e = |e: std::io::Error| format!("dinput8.dll: {e}");
    if on {
        if off.exists() && !live.exists() {
            fs::rename(&off, &live).map_err(e)?;
        }
    } else if live.exists() {
        if off.exists() {
            fs::remove_file(&off).map_err(e)?;
        }
        fs::rename(&live, &off).map_err(e)?;
    }
    Ok(())
}

pub fn expected_ext(game: &Game, dir: &Path, st: &GameState) -> String {
    if game.key == "ds2" {
        return if st.mods.get("rando").copied().unwrap_or(false) {
            "co2"
        } else {
            "va2"
        }
        .into();
    }
    match ini::get(&settings_ini(game, dir), "save_file_extension") {
        Some(e) if !e.is_empty() && e != "sl2" => e,
        _ => "co2".into(),
    }
}

/// Runs in a worker thread (may wait for Steam to start). Returns a message for the UI.
pub fn launch(
    game: &Game,
    loc: &platform::Located,
    st: &GameState,
    save_base: Option<&Path>,
    mirrored: &std::collections::BTreeMap<String, String>,
) -> Result<String, String> {
    if platform::game_running(game) {
        return Err(format!("{} is already running.", game.name));
    }
    let dir = &loc.game_dir;
    let root = save_base.and_then(|b| saves::find_root(game, b));
    let pre_backup = || {
        if let Some(r) = &root {
            let _ = saves::backup(game, r, "pre-launch");
        }
    };

    if st.mode == Mode::Vanilla {
        set_dll(dir, false)?;
        pre_backup();
        platform::launch_via_steam(game)?;
        return Ok(format!("Starting {} (vanilla, no mods)…", game.name));
    }

    if !has_seamless(game, dir) {
        return Err("Seamless Co-op is not installed for this game.".into());
    }
    let ini_path = settings_ini(game, dir);
    let ext = expected_ext(game, dir, st);
    let ie = |e: std::io::Error| format!("Could not update settings: {e}");
    match game.key {
        "ds2" => {
            set_dll(dir, true)?;
            let rando = st.mods.get("rando").copied().unwrap_or(false);
            let mi = dir.join("modengine.ini");
            if mi.exists() {
                ini::set(
                    &mi,
                    "useModOverrideDirectory",
                    if rando { "1" } else { "0" },
                )
                .map_err(ie)?;
            }
            ini::set(&ini_path, "save_file_extension", &ext).map_err(ie)?;
        }
        key => {
            if key == "ds3" {
                set_dll(dir, st.mods.get("honest").copied().unwrap_or(false))?;
            }
            ini::set(&ini_path, "save_file_extension", &ext).map_err(ie)?;
        }
    }
    shared::apply_to_file(&ini_path, mirrored);
    // never let a modded session touch the normal save
    match ini::get(&ini_path, "save_file_extension") {
        Some(e) if !e.is_empty() && e != "sl2" => {}
        _ => {
            return Err(
                "Refusing to start modded: the mod would use your normal .sl2 save.".into(),
            );
        }
    }
    pre_backup();
    platform::run_exe(game, loc, &format!("{}_launcher.exe", game.sc), game.key)?;
    Ok(format!(
        "Starting {} with Seamless Co-op (save .{ext})…",
        game.name
    ))
}
