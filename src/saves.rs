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
    pub name: String,
    pub files: Vec<PathBuf>,
}

pub fn fmt_time(t: SystemTime) -> String {
    DateTime::<Local>::from(t).format("%Y-%m-%d %H:%M").to_string()
}

pub fn children_dirs(dir: &Path) -> Vec<PathBuf> {
    children(dir).into_iter().filter(|p| p.is_dir()).collect()
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.sort();
    v
}

fn is_save(game: &Game, p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with(&format!("{}.", game.save_file)) && !name.contains(".bak") && p.is_file()
}

pub fn find_root(game: &Game, base: &Path) -> Option<PathBuf> {
    let has = |d: &Path| children(d).iter().any(|p| is_save(game, p));
    if has(base) {
        return Some(base.to_path_buf());
    }
    children(base).into_iter().find(|d| d.is_dir() && has(d))
}

pub fn list(game: &Game, root: &Path) -> Vec<SaveFile> {
    let mut v: Vec<SaveFile> = children(root)
        .into_iter()
        .filter(|p| is_save(game, p))
        .filter_map(|p| {
            let md = p.metadata().ok()?;
            Some(SaveFile { ext: p.extension()?.to_string_lossy().into(), size: md.len(), modified: md.modified().ok()?, path: p })
        })
        .collect();
    v.sort_by(|a, b| a.ext.cmp(&b.ext));
    v
}

pub fn backup_root(game: &Game) -> PathBuf {
    dirs::document_dir().or_else(|| dirs::home_dir().map(|h| h.join("Documents"))).unwrap_or_default().join(game.backups)
}

pub fn backup(game: &Game, root: &Path, note: &str) -> io::Result<Option<String>> {
    let files = list(game, root);
    if files.is_empty() {
        return Ok(None);
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let name = if note.is_empty() { stamp.to_string() } else { format!("{stamp} {note}") };
    let dest = backup_root(game).join(&name);
    fs::create_dir_all(&dest)?;
    for f in &files {
        fs::copy(&f.path, dest.join(f.path.file_name().unwrap()))?;
    }
    if note == "pre-launch" {
        let mut old: Vec<PathBuf> = children(&backup_root(game)).into_iter().filter(|p| p.to_string_lossy().ends_with("pre-launch")).collect();
        old.reverse();
        old.iter().skip(10).for_each(|p| drop(fs::remove_dir_all(p)));
    }
    Ok(Some(name))
}

pub fn list_backups(game: &Game, limit: usize) -> Vec<Backup> {
    let mut dirs: Vec<PathBuf> = children(&backup_root(game)).into_iter().filter(|p| p.is_dir()).collect();
    dirs.reverse();
    dirs.into_iter().take(limit).map(|d| Backup { name: d.file_name().unwrap().to_string_lossy().into(), files: children(&d).into_iter().filter(|p| p.is_file()).collect() }).collect()
}

pub fn restorable(b: &Backup) -> Vec<&PathBuf> {
    b.files.iter().filter(|p| !p.to_string_lossy().contains(".bak")).collect()
}

pub fn restore(game: &Game, root: &Path, file: &Path) -> io::Result<()> {
    let target = root.join(file.file_name().unwrap());
    if target.exists() {
        backup(game, root, "before-restore")?;
    }
    fs::copy(file, target).map(drop)
}

pub fn copy_over(game: &Game, root: &Path, src: &Path, ext: &str) -> io::Result<()> {
    let dst = root.join(format!("{}.{ext}", game.save_file));
    if dst == src {
        return Ok(());
    }
    backup(game, root, "before-copy")?;
    fs::copy(src, dst).map(drop)
}

pub fn auto_backups(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<(SystemTime, PathBuf)> = children(root)
        .into_iter()
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.ends_with(".bak") && n.len() > 14 && n.as_bytes()[..6].iter().all(u8::is_ascii_digit) && n.as_bytes()[6] == b'_'
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|x| x.1).collect()
}
