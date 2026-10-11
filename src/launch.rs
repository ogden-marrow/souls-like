use crate::{
    config::{GameState, Mode},
    games::Game,
    ini, platform, saves, shared,
};
use std::{collections::BTreeMap, fs, io, path::Path, path::PathBuf};

pub fn settings_path(game: &Game, dir: &Path) -> PathBuf {
    dir.join("SeamlessCoop").join(format!("{}_settings.ini", game.sc))
}

pub fn has_seamless(game: &Game, dir: &Path) -> bool {
    dir.join(format!("{}_launcher.exe", game.sc)).exists() && settings_path(game, dir).exists()
}

fn set_loader(dir: &Path, on: bool) -> io::Result<()> {
    let (live, off) = (dir.join("dinput8.dll"), dir.join("dinput8.dll.off"));
    if on && off.exists() && !live.exists() {
        fs::rename(&off, &live)?;
    } else if !on && live.exists() {
        let _ = fs::remove_file(&off);
        fs::rename(&live, &off)?;
    }
    Ok(())
}

fn enabled(st: &GameState, id: &str) -> bool {
    st.mods.get(id).copied().unwrap_or(false)
}

pub fn expected_ext(game: &Game, dir: &Path, st: &GameState) -> String {
    if game.key == "ds2" {
        return if enabled(st, "rando") { "co2" } else { "va2" }.into();
    }
    ini::get(&settings_path(game, dir), "save_file_extension").filter(|e| !e.is_empty() && e != "sl2").unwrap_or("co2".into())
}

pub fn launch(game: &Game, loc: &platform::Located, st: &GameState, base: Option<&Path>, mirrored: &BTreeMap<String, String>) -> Result<String, String> {
    if platform::running(&[game.exe])[0] {
        return Err(format!("{} is already running.", game.name));
    }
    let dir = &loc.game_dir;
    let loader = |on| set_loader(dir, on).map_err(|e| format!("dinput8.dll: {e}"));
    let backup = || base.and_then(|b| saves::find_root(game, b)).into_iter().for_each(|r| drop(saves::backup(game, &r, "pre-launch")));

    if st.mode == Mode::Vanilla {
        loader(false)?;
        backup();
        platform::launch_via_steam(game)?;
        return Ok(format!("Starting {} (vanilla, no mods)…", game.name));
    }
    if !has_seamless(game, dir) {
        return Err("Seamless Co-op is not installed for this game.".into());
    }
    let (ini_path, ext) = (settings_path(game, dir), expected_ext(game, dir, st));
    let io_err = |e: io::Error| format!("Could not update settings: {e}");
    match game.key {
        "ds2" => {
            loader(true)?;
            let engine = dir.join("modengine.ini");
            if engine.exists() {
                ini::set(&engine, "useModOverrideDirectory", if enabled(st, "rando") { "1" } else { "0" }).map_err(io_err)?;
            }
        }
        "ds3" => loader(enabled(st, "honest"))?,
        _ => {}
    }
    ini::set(&ini_path, "save_file_extension", &ext).map_err(io_err)?;
    shared::apply_to_file(&ini_path, mirrored);
    if ini::get(&ini_path, "save_file_extension").is_none_or(|e| e.is_empty() || e == "sl2") {
        return Err("Refusing to start modded: the mod would use your normal .sl2 save.".into());
    }
    backup();
    platform::run_exe(game, loc, &format!("{}_launcher.exe", game.sc))?;
    Ok(format!("Starting {} with Seamless Co-op (save .{ext})…", game.name))
}
