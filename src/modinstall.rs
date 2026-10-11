//! Drag-and-drop mod installer. A zip is recognised by a marker file inside it, then its files are
//! copied into the right game folder. Replaced files are backed up and users' settings files are kept.

use crate::games::GAMES;
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

/// (marker file name in lower case, game index, mod title)
const MARKERS: &[(&str, usize, &str)] = &[
    ("ds1sc_launcher.exe", 0, "Seamless Co-op"),
    ("ds2sc_launcher.exe", 1, "Seamless Co-op"),
    ("ds3sc_launcher.exe", 2, "Seamless Co-op"),
    ("honestmerchant.ini", 2, "Honest Merchant"),
];

pub struct Plan {
    pub zip: PathBuf,
    pub game: usize,
    pub title: &'static str,
    pub marker: String,
    /// (index in the zip, destination relative to the game folder)
    pub entries: Vec<(usize, PathBuf)>,
}

impl Plan {
    pub fn describe(&self) -> String {
        format!(
            "{} for {} · {} files",
            self.title,
            GAMES[self.game].name,
            self.entries.len()
        )
    }
}

fn safe_rel(p: &Path) -> bool {
    p.components().all(|c| matches!(c, Component::Normal(_)))
}

fn junk(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.starts_with("__MACOSX") || s.ends_with(".DS_Store") || s.ends_with("Thumbs.db")
}

pub fn inspect(zip_path: &Path) -> Result<Plan, String> {
    let f = fs::File::open(zip_path).map_err(|e| format!("Cannot open file: {e}"))?;
    let mut ar = zip::ZipArchive::new(f).map_err(|_| "Not a zip file.".to_string())?;
    let mut names: Vec<(usize, PathBuf)> = Vec::new();
    for i in 0..ar.len() {
        let e = ar.by_index(i).map_err(|e| e.to_string())?;
        if e.is_dir() {
            continue;
        }
        let Some(name) = e.enclosed_name() else {
            return Err("The zip contains an unsafe path; refusing to install it.".into());
        };
        if let Some(mode) = e.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err("The zip contains a symbolic link; refusing to install it.".into());
            }
        }
        if !junk(&name) {
            names.push((i, name));
        }
    }
    for (marker, game, title) in MARKERS {
        let hit = names.iter().find(|(_, n)| {
            n.file_name()
                .map(|f| f.to_string_lossy().to_lowercase() == *marker)
                .unwrap_or(false)
        });
        let Some((_, mpath)) = hit else { continue };
        let base = mpath.parent().map(Path::to_path_buf).unwrap_or_default();
        let entries: Vec<(usize, PathBuf)> = names
            .iter()
            .filter_map(|(i, n)| {
                n.strip_prefix(&base)
                    .ok()
                    .map(|rel| (*i, rel.to_path_buf()))
            })
            .filter(|(_, rel)| safe_rel(rel) && !rel.as_os_str().is_empty())
            .collect();
        return Ok(Plan {
            zip: zip_path.to_path_buf(),
            game: *game,
            title,
            marker: (*marker).to_string(),
            entries,
        });
    }
    Err("Not recognised. Souls-like installs Seamless Co-op (DS1, DS2, DS3) and Honest Merchant zips.".into())
}

pub fn install(plan: &Plan, game_dir: &Path) -> Result<String, String> {
    let f = fs::File::open(&plan.zip).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_root = game_dir.join(".souls-like-backup").join(&stamp);
    let (mut written, mut replaced, mut kept) = (0, 0, 0);
    for (idx, rel) in &plan.entries {
        let dest = game_dir.join(rel);
        let lower = rel.to_string_lossy().to_lowercase();
        if lower.ends_with("_settings.ini") && dest.exists() {
            kept += 1; // never overwrite the user's own settings
            continue;
        }
        if dest.exists() {
            let b = backup_root.join(rel);
            if let Some(d) = b.parent() {
                fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            fs::copy(&dest, &b).map_err(|e| format!("Could not back up {}: {e}", rel.display()))?;
            replaced += 1;
        }
        if let Some(d) = dest.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut entry = ar.by_index(*idx).map_err(|e| e.to_string())?;
        let mut out = fs::File::create(&dest)
            .map_err(|e| format!("Could not write {}: {e}", rel.display()))?;
        io::copy(&mut entry.by_ref(), &mut out).map_err(|e| e.to_string())?;
        written += 1;
    }
    Ok(format!(
        "Installed {} for {}: {written} files{}{}.",
        plan.title,
        GAMES[plan.game].name,
        if replaced > 0 {
            format!(", {replaced} replaced (backed up)")
        } else {
            String::new()
        },
        if kept > 0 {
            format!(", your settings file kept")
        } else {
            String::new()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = fs::File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        for (n, data) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn detects_game_and_keeps_settings() {
        let dir = std::env::temp_dir().join("souls-like-mod-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("game/SeamlessCoop")).unwrap();
        make_zip(
            &dir.join("m.zip"),
            &[
                ("Pack/ds3sc_launcher.exe", b"exe"),
                ("Pack/SeamlessCoop/ds3sc.dll", b"dll"),
                ("Pack/SeamlessCoop/ds3sc_settings.ini", b"new"),
            ],
        );
        fs::write(dir.join("game/SeamlessCoop/ds3sc_settings.ini"), b"mine").unwrap();
        let plan = inspect(&dir.join("m.zip")).unwrap();
        assert_eq!(plan.game, 2);
        assert_eq!(plan.entries.len(), 3);
        install(&plan, &dir.join("game")).unwrap();
        assert_eq!(
            fs::read(dir.join("game/SeamlessCoop/ds3sc_settings.ini")).unwrap(),
            b"mine"
        );
        assert!(dir.join("game/ds3sc_launcher.exe").exists());
    }

    #[test]
    fn rejects_unknown_zip() {
        let dir = std::env::temp_dir().join("souls-like-mod-test2");
        fs::create_dir_all(&dir).unwrap();
        make_zip(&dir.join("x.zip"), &[("readme.txt", b"hi")]);
        assert!(inspect(&dir.join("x.zip")).is_err());
    }
}
