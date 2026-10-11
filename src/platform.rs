//! Everything that differs per OS: finding Steam, where saves live, detecting running games and
//! starting them (native on Windows, through Proton on Linux; macOS has no runner yet).

use crate::games::{Game, SaveBase};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread::sleep,
    time::Duration,
};

pub const OS_SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "windows"));

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

// ------------------------------------------------------------------ Steam discovery
pub fn steam_roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(p) = k.get_value::<String, _>("SteamPath") {
                v.push(PathBuf::from(p));
            }
        }
        v.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    }
    #[cfg(target_os = "linux")]
    {
        let h = home();
        v.push(h.join("snap/steam/common/.local/share/Steam"));
        v.push(h.join(".local/share/Steam"));
        v.push(h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    #[cfg(target_os = "macos")]
    v.push(home().join("Library/Application Support/Steam"));
    v.into_iter()
        .filter(|p| p.join("steamapps").is_dir())
        .collect()
}

/// All `steamapps` folders: every Steam root plus the extra libraries listed in `libraryfolders.vdf`.
pub fn libraries() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(p) = env::var("SOULS_LIKE_LIBRARY") {
        out.push(PathBuf::from(p).join("steamapps"));
    }
    for root in steam_roots() {
        out.push(root.join("steamapps"));
        if let Ok(txt) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in txt.lines() {
                let parts: Vec<&str> = line.split('"').collect();
                // "<tab>"path"<tab><tab>"/some/where"
                if parts.len() >= 4 && parts[1] == "path" {
                    out.push(PathBuf::from(parts[3].replace("\\\\", "\\")).join("steamapps"));
                }
            }
        }
    }
    let mut seen = Vec::new();
    out.retain(|p| {
        let k = fs::canonicalize(p).unwrap_or(p.clone());
        let new = !seen.contains(&k);
        seen.push(k);
        new && p.is_dir()
    });
    out
}

/// Where a game is installed.
#[derive(Clone)]
pub struct Located {
    pub steamapps: PathBuf,
    pub game_dir: PathBuf,
}

pub fn locate(game: &Game, game_dir_override: &str) -> Option<Located> {
    let libs = libraries();
    let lib = libs
        .iter()
        .find(|l| l.join(format!("appmanifest_{}.acf", game.appid)).exists())?
        .clone();
    let game_dir = if game_dir_override.trim().is_empty() {
        let mut p = lib.join("common");
        for part in game.dir {
            p.push(part);
        }
        p
    } else {
        PathBuf::from(game_dir_override.trim())
    };
    game_dir.is_dir().then_some(Located {
        steamapps: lib,
        game_dir,
    })
}

// ------------------------------------------------------------------ save folder
/// The folder that contains the per-SteamID save folders (before `find_root`).
pub fn save_base_dir(game: &Game, loc: Option<&Located>, override_dir: &str) -> Option<PathBuf> {
    if !override_dir.trim().is_empty() {
        return Some(PathBuf::from(override_dir.trim()));
    }
    #[cfg(target_os = "windows")]
    {
        let _ = loc;
        let base = match game.save_base {
            SaveBase::Documents => dirs::document_dir()?,
            SaveBase::Roaming => dirs::config_dir()?,
        };
        Some(base.join(game.save_rel))
    }
    #[cfg(target_os = "linux")]
    {
        let user = loc?.steamapps.join(format!(
            "compatdata/{}/pfx/drive_c/users/steamuser",
            game.appid
        ));
        let base = match game.save_base {
            SaveBase::Documents => user.join("Documents"),
            SaveBase::Roaming => user.join("AppData/Roaming"),
        };
        Some(base.join(game.save_rel))
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (game, loc, SaveBase::Documents);
        None
    }
}

// ------------------------------------------------------------------ processes
fn quiet(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
}

pub fn game_running(game: &Game) -> bool {
    process_running(game.proc_name)
}

#[cfg(unix)]
fn process_running(name: &str) -> bool {
    quiet(Command::new("pgrep").args(["-f", name]))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn process_running(name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {name}"), "/NH"])
        .creation_flags(0x0800_0000)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .to_lowercase()
                .contains(&name.to_lowercase())
        })
        .unwrap_or(false)
}

fn steam_running() -> bool {
    #[cfg(target_os = "linux")]
    return process_running("ubuntu12_32/steam");
    #[cfg(target_os = "windows")]
    return process_running("steam.exe");
    #[cfg(target_os = "macos")]
    return process_running("steam_osx");
}

// ------------------------------------------------------------------ launching
/// Start the plain game through Steam.
pub fn launch_via_steam(game: &Game) -> Result<(), String> {
    let uri = format!("steam://rungameid/{}", game.appid);
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", &uri])
            .creation_flags(0x0800_0000);
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = Command::new("steam");
        c.arg(&uri);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(&uri);
        c
    };
    quiet(&mut cmd)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not start Steam: {e}"))
}

fn ensure_steam() -> Result<(), String> {
    if steam_running() {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    let _ = quiet(Command::new("cmd").args(["/C", "start", "", "steam://open/main"])).spawn();
    #[cfg(target_os = "linux")]
    let _ = quiet(Command::new("steam").arg("-silent")).spawn();
    for _ in 0..40 {
        sleep(Duration::from_secs(1));
        if steam_running() {
            sleep(Duration::from_secs(6));
            return Ok(());
        }
    }
    Err("Steam did not start.".into())
}

/// Run a Windows exe from the game folder: directly on Windows, inside the game's Proton prefix on Linux.
pub fn run_exe(game: &Game, loc: &Located, exe: &str, log_name: &str) -> Result<(), String> {
    if !OS_SUPPORTED {
        return Err("Launching games is not supported on this OS yet.".into());
    }
    ensure_steam()?;
    let exe_path = loc.game_dir.join(exe);
    let _ = fs::create_dir_all(crate::config::log_dir());
    let log = fs::File::create(crate::config::log_dir().join(format!("{log_name}.log")))
        .map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    {
        let _ = game;
        use std::os::windows::process::CommandExt;
        return Command::new(&exe_path)
            .current_dir(&loc.game_dir)
            .stdout(log)
            .stderr(err)
            .creation_flags(0x0800_0000)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not start {exe}: {e}"));
    }

    #[cfg(target_os = "linux")]
    {
        let root = steam_roots()
            .into_iter()
            .next()
            .ok_or("Steam install not found.")?;
        let proton = find_proton(&root, &loc.steamapps)
            .ok_or("No Proton found. Enable Steam Play for all titles in Steam settings.")?;
        let prefix = loc.steamapps.join(format!("compatdata/{}", game.appid));
        if !prefix.join("pfx").is_dir() {
            return Err("No Proton prefix yet. Launch the game once from Steam first.".into());
        }
        let mut cmd = Command::new(proton.join("proton"));
        cmd.arg("run")
            .arg(&exe_path)
            .current_dir(&loc.game_dir)
            .env("STEAM_COMPAT_DATA_PATH", &prefix)
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &root)
            .env("SteamAppId", game.appid.to_string())
            .env("SteamGameId", game.appid.to_string())
            .env("WINEDLLOVERRIDES", "dinput8=n,b")
            .stdout(log)
            .stderr(err);
        // snap Steam keeps its connection pipe (~/.steam/steam.pipe) under its own home
        let snap_home = home().join("snap/steam/common");
        if !home().join(".steam/steam.pipe").exists() && snap_home.is_dir() {
            cmd.env("HOME", snap_home);
        }
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        return cmd
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not start Proton: {e}"));
    }

    #[cfg(target_os = "macos")]
    {
        let _ = (game, exe_path, log, err);
        Err("Launching games is not supported on macOS yet.".into())
    }
}

#[cfg(target_os = "linux")]
fn find_proton(root: &Path, steamapps: &Path) -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    for base in [
        root.join("steamapps/common"),
        steamapps.join("common"),
        root.join("compatibilitytools.d"),
    ] {
        let custom = base.ends_with("compatibilitytools.d");
        if let Ok(rd) = fs::read_dir(&base) {
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if p.join("proton").exists()
                    && (custom || name.starts_with("Proton"))
                    && !cands.contains(&p)
                {
                    cands.push(p);
                }
            }
        }
    }
    if let Some(p) = cands.iter().find(|p| p.ends_with("Proton - Experimental")) {
        return Some(p.clone());
    }
    cands.sort_by_key(|p| natural_key(&p.file_name().unwrap().to_string_lossy()));
    cands.pop()
}

#[cfg(target_os = "linux")]
fn natural_key(s: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for c in s.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{digits:0>6}"));
                digits.clear();
            }
            out.push(c);
        }
    }
    out
}

pub fn open_path(p: &Path) {
    let _ = fs::create_dir_all(if p.extension().is_some() {
        p.parent().unwrap_or(p)
    } else {
        p
    });
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "linux")]
    let _ = quiet(Command::new("xdg-open").arg(p)).spawn();
    #[cfg(target_os = "macos")]
    let _ = quiet(Command::new("open").arg(p)).spawn();
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::{games::GAMES, saves};

    /// Prints what the app sees on this machine: `cargo test live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_smoke() {
        println!("libraries: {:?}", libraries());
        for g in &GAMES {
            let loc = locate(g, "");
            let base = save_base_dir(g, loc.as_ref(), "");
            let root = base.as_ref().and_then(|b| saves::find_root(g, b));
            let n = root.as_ref().map(|r| saves::list(g, r).len());
            println!(
                "{}: dir={:?} saves_root={:?} saves={:?} running={}",
                g.name,
                loc.map(|l| l.game_dir),
                root,
                n,
                game_running(g)
            );
        }
    }
}
