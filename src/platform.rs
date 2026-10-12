use crate::games::{GAMES, Game};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread::sleep,
    time::Duration,
};

pub const CAN_LAUNCH: bool = cfg!(any(target_os = "linux", target_os = "windows"));

const STEAM_PROCESS: &str = if cfg!(windows) {
    "steam.exe"
} else if cfg!(target_os = "macos") {
    "steam_osx"
} else {
    "ubuntu12_32/steam"
};

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn quiet(cmd: &mut Command) -> &mut Command {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(cmd, 0x0800_0000);
    cmd
}

pub fn steam_roots() -> Vec<PathBuf> {
    let h = home();
    let mut v: Vec<PathBuf> = vec![];
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        let path = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").and_then(|k| k.get_value::<String, _>("SteamPath"));
        v.extend(path.map(PathBuf::from));
        v.push(r"C:\Program Files (x86)\Steam".into());
    }
    if cfg!(target_os = "linux") {
        v.extend(["snap/steam/common/.local/share/Steam", ".local/share/Steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"].map(|p| h.join(p)));
    }
    if cfg!(target_os = "macos") {
        v.push(h.join("Library/Application Support/Steam"));
    }
    v.into_iter().filter(|p| p.join("steamapps").is_dir()).collect()
}

pub fn libraries() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = env::var("SOULS_LIKE_LIBRARY").into_iter().map(|p| PathBuf::from(p).join("steamapps")).collect();
    for root in steam_roots() {
        out.push(root.join("steamapps"));
        let vdf = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
        for parts in vdf.lines().map(|l| l.split('"').collect::<Vec<_>>()).filter(|p| p.len() >= 4 && p[1] == "path") {
            out.push(PathBuf::from(parts[3].replace("\\\\", "\\")).join("steamapps"));
        }
    }
    let mut seen = vec![];
    out.retain(|p| {
        let canonical = fs::canonicalize(p).unwrap_or(p.clone());
        let new = !seen.contains(&canonical);
        seen.push(canonical);
        new && p.is_dir()
    });
    out
}

#[derive(Clone)]
pub struct Located {
    pub steamapps: PathBuf,
    pub game_dir: PathBuf,
}

pub fn locate(game: &Game, game_dir_override: &str) -> Option<Located> {
    let steamapps = libraries().into_iter().find(|l| l.join(format!("appmanifest_{}.acf", game.appid)).exists())?;
    let custom = game_dir_override.trim();
    let game_dir = if custom.is_empty() { game.dir.iter().fold(steamapps.join("common"), |p, part| p.join(part)) } else { PathBuf::from(custom) };
    game_dir.is_dir().then_some(Located { steamapps, game_dir })
}

pub fn save_base_dir(game: &Game, loc: Option<&Located>, override_dir: &str) -> Option<PathBuf> {
    if !override_dir.trim().is_empty() {
        return Some(PathBuf::from(override_dir.trim()));
    }
    let base = if cfg!(windows) {
        if game.roaming { dirs::config_dir()? } else { dirs::document_dir()? }
    } else if cfg!(target_os = "linux") {
        let user = loc?.steamapps.join(format!("compatdata/{}/pfx/drive_c/users/steamuser", game.appid));
        user.join(if game.roaming { "AppData/Roaming" } else { "Documents" })
    } else {
        return None;
    };
    Some(base.join(game.save_rel))
}

pub fn running(names: &[&str]) -> Vec<bool> {
    #[cfg(target_os = "linux")]
    {
        let mut found = vec![false; names.len()];
        let me = std::process::id().to_string();
        for e in fs::read_dir("/proc").into_iter().flatten().flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().filter(|s| s.bytes().all(|b| b.is_ascii_digit()) && *s != me) else { continue };
            let Ok(cmd) = fs::read(format!("/proc/{pid}/cmdline")) else { continue };
            for (f, n) in found.iter_mut().zip(names) {
                *f = *f || cmd.windows(n.len()).any(|w| w == n.as_bytes());
            }
            if found.iter().all(|f| *f) {
                break;
            }
        }
        found
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
        };
        let wanted: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
        let mut found = vec![false; names.len()];
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry);
            while more != 0 {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                let exe = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                for (f, w) in found.iter_mut().zip(&wanted) {
                    *f |= exe == *w;
                }
                more = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
        }
        found
    }
    #[cfg(target_os = "macos")]
    names.iter().map(|n| quiet(Command::new("pgrep").args(["-f", n])).status().is_ok_and(|s| s.success())).collect()
}

pub fn running_games() -> [bool; 3] {
    let r = running(&GAMES.each_ref().map(|g| g.exe));
    [r[0], r[1], r[2]]
}

pub fn launch_via_steam(game: &Game) -> Result<(), String> {
    let uri = format!("steam://rungameid/{}", game.appid);
    let mut cmd = Command::new(if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "steam"
    });
    cmd.arg(&uri);
    hidden(quiet(&mut cmd)).spawn().map(drop).map_err(|e| format!("Could not start Steam: {e}"))
}

fn ensure_steam() -> Result<(), String> {
    if running(&[STEAM_PROCESS])[0] {
        return Ok(());
    }
    let _ = if cfg!(windows) { hidden(quiet(Command::new("cmd").args(["/C", "start", "", "steam://open/main"]))).spawn() } else { quiet(Command::new("steam").arg("-silent")).spawn() };
    for _ in 0..40 {
        sleep(Duration::from_secs(1));
        if running(&[STEAM_PROCESS])[0] {
            sleep(Duration::from_secs(6));
            return Ok(());
        }
    }
    Err("Steam did not start.".into())
}

pub fn run_exe(game: &Game, loc: &Located, exe: &str) -> Result<(), String> {
    if !CAN_LAUNCH {
        return Err("Launching games is not supported on this OS yet.".into());
    }
    ensure_steam()?;
    let _ = fs::create_dir_all(crate::config::log_dir());
    let log = fs::File::create(crate::config::log_dir().join(format!("{}.log", game.key))).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let exe_path = loc.game_dir.join(exe);

    #[cfg(windows)]
    let mut cmd = Command::new(&exe_path);
    #[cfg(not(windows))]
    let mut cmd = {
        let root = steam_roots().into_iter().next().ok_or("Steam install not found.")?;
        let proton = find_proton(&root, &loc.steamapps).ok_or("No Proton found. Enable Steam Play for all titles in Steam settings.")?;
        let prefix = loc.steamapps.join(format!("compatdata/{}", game.appid));
        if !prefix.join("pfx").is_dir() {
            return Err("No Proton prefix yet. Launch the game once from Steam first.".into());
        }
        let mut c = Command::new(proton.join("proton"));
        c.arg("run")
            .arg(&exe_path)
            .env("STEAM_COMPAT_DATA_PATH", &prefix)
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &root)
            .env("SteamAppId", game.appid.to_string())
            .env("SteamGameId", game.appid.to_string())
            .env("WINEDLLOVERRIDES", "dinput8=n,b");
        let snap_home = home().join("snap/steam/common");
        if !home().join(".steam/steam.pipe").exists() && snap_home.is_dir() {
            c.env("HOME", snap_home);
        }
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut c, 0);
        c
    };
    hidden(cmd.current_dir(&loc.game_dir).stdout(log).stderr(err)).spawn().map(drop).map_err(|e| format!("Could not start {exe}: {e}"))
}

#[cfg(not(windows))]
fn find_proton(root: &Path, steamapps: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = vec![];
    for base in [root.join("steamapps/common"), steamapps.join("common"), root.join("compatibilitytools.d")] {
        let custom = base.ends_with("compatibilitytools.d");
        for p in fs::read_dir(&base).into_iter().flatten().flatten().map(|e| e.path()) {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if p.join("proton").exists() && (custom || name.starts_with("Proton")) && !found.contains(&p) {
                found.push(p);
            }
        }
    }
    let version = |p: &PathBuf| -> Vec<u32> { p.to_string_lossy().split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect() };
    found.iter().find(|p| p.ends_with("Proton - Experimental")).cloned().or_else(|| found.into_iter().max_by_key(version))
}

pub fn open_path(p: &Path) {
    let _ = fs::create_dir_all(if p.extension().is_some() { p.parent().unwrap_or(p) } else { p });
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = hidden(quiet(Command::new(opener).arg(p))).spawn();
}
