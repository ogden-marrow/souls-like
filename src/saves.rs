//! Save files and backups. A "save root" is the per-user folder (named by SteamID) that holds the saves.

use crate::games::Game;
use chrono::{DateTime, Local};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub struct SaveFile {
    pub path: PathBuf,
    pub ext: String,
    pub size: u64,
    pub modified: SystemTime,
}

pub struct Backup {
    pub dir: PathBuf,
    pub name: String,
    pub files: Vec<PathBuf>,
}

pub fn fmt_time(t: SystemTime) -> String {
    DateTime::<Local>::from(t)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn is_save(game: &Game, p: &Path) -> bool {
    let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with(&format!("{}.", game.save_file)) && !name.contains(".bak") && p.is_file()
}

fn has_saves(game: &Game, dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| is_save(game, &e.path())))
        .unwrap_or(false)
}

/// First subfolder of `base` that contains saves, or `base` itself if it has them directly.
pub fn find_root(game: &Game, base: &Path) -> Option<PathBuf> {
    if has_saves(game, base) {
        return Some(base.to_path_buf());
    }
    let mut subs: Vec<PathBuf> = fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();
    subs.into_iter().find(|d| has_saves(game, d))
}

pub fn list(game: &Game, root: &Path) -> Vec<SaveFile> {
    let mut v: Vec<SaveFile> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_save(game, p))
        .filter_map(|p| {
            let md = p.metadata().ok()?;
            Some(SaveFile {
                ext: p.extension()?.to_string_lossy().to_string(),
                size: md.len(),
                modified: md.modified().ok()?,
                path: p,
            })
        })
        .collect();
    v.sort_by(|a, b| a.ext.cmp(&b.ext));
    v
}

pub fn backup_root(game: &Game) -> PathBuf {
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Documents")))
        .unwrap_or_default()
        .join(game.backups)
}

/// Copy every save into a new timestamped folder. Returns its name.
pub fn backup(game: &Game, root: &Path, note: &str) -> io::Result<Option<String>> {
    let files = list(game, root);
    if files.is_empty() {
        return Ok(None);
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = if note.is_empty() {
        stamp
    } else {
        format!("{stamp} {note}")
    };
    let dest = backup_root(game).join(&name);
    fs::create_dir_all(&dest)?;
    for f in &files {
        fs::copy(&f.path, dest.join(f.path.file_name().unwrap()))?;
    }
    if note == "pre-launch" {
        let mut old: Vec<String> = fs::read_dir(backup_root(game))?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with("pre-launch"))
            .collect();
        old.sort();
        old.reverse();
        for n in old.into_iter().skip(10) {
            let _ = fs::remove_dir_all(backup_root(game).join(n));
        }
    }
    Ok(Some(name))
}

pub fn list_backups(game: &Game, limit: usize) -> Vec<Backup> {
    let Ok(rd) = fs::read_dir(backup_root(game)) else {
        return vec![];
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.reverse();
    dirs.into_iter()
        .take(limit)
        .map(|d| {
            let mut files: Vec<PathBuf> = fs::read_dir(&d)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            Backup {
                name: d.file_name().unwrap().to_string_lossy().to_string(),
                dir: d,
                files,
            }
        })
        .collect()
}

/// Backup files that are real saves (not the mod's `.bak` auto-backups).
pub fn restorable<'a>(b: &'a Backup) -> Vec<&'a PathBuf> {
    b.files
        .iter()
        .filter(|p| !p.file_name().unwrap().to_string_lossy().contains(".bak"))
        .collect()
}

pub fn restore(game: &Game, root: &Path, file: &Path) -> io::Result<()> {
    let target = root.join(file.file_name().unwrap());
    if target.exists() {
        backup(game, root, "before-restore")?;
    }
    fs::copy(file, target).map(|_| ())
}

pub fn copy_over(game: &Game, root: &Path, src: &Path, ext: &str) -> io::Result<()> {
    let dst = root.join(format!("{}.{}", game.save_file, ext));
    if dst == src {
        return Ok(());
    }
    backup(game, root, "before-copy")?;
    fs::copy(src, dst).map(|_| ())
}

/// The Seamless Co-op mod writes `DDMMYY_HHMMSS_<save>.bak` files next to the save.
pub fn auto_backups(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<(SystemTime, PathBuf)> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.ends_with(".bak")
                && n.len() > 14
                && n.as_bytes()[..6].iter().all(u8::is_ascii_digit)
                && n.as_bytes()[6] == b'_'
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, p)| p).collect()
}
