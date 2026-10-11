use crate::games::GAMES;
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

const MARKERS: [(&str, usize, &str); 4] =
    [("ds1sc_launcher.exe", 0, "Seamless Co-op"), ("ds2sc_launcher.exe", 1, "Seamless Co-op"), ("ds3sc_launcher.exe", 2, "Seamless Co-op"), ("honestmerchant.ini", 2, "Honest Merchant")];

pub struct Plan {
    pub zip: PathBuf,
    pub game: usize,
    pub title: &'static str,
    pub marker: &'static str,
    pub entries: Vec<(usize, PathBuf)>,
}

impl Plan {
    pub fn describe(&self) -> String {
        format!("{} for {} · {} files", self.title, GAMES[self.game].name, self.entries.len())
    }
}

fn open(path: &Path) -> Result<zip::ZipArchive<fs::File>, String> {
    zip::ZipArchive::new(fs::File::open(path).map_err(|e| format!("Cannot open file: {e}"))?).map_err(|_| "Not a zip file.".to_string())
}

pub fn inspect(zip_path: &Path) -> Result<Plan, String> {
    let mut ar = open(zip_path)?;
    let mut names: Vec<(usize, PathBuf)> = vec![];
    for i in 0..ar.len() {
        let e = ar.by_index(i).map_err(|e| e.to_string())?;
        if e.is_dir() {
            continue;
        }
        let name = e.enclosed_name().ok_or("The zip contains an unsafe path; refusing to install it.")?;
        if e.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("The zip contains a symbolic link; refusing to install it.".into());
        }
        let s = name.to_string_lossy();
        if !(s.starts_with("__MACOSX") || s.ends_with(".DS_Store") || s.ends_with("Thumbs.db")) {
            names.push((i, name));
        }
    }
    for (marker, game, title) in MARKERS {
        let Some((_, found)) = names.iter().find(|(_, n)| n.file_name().is_some_and(|f| f.to_string_lossy().to_lowercase() == marker)) else {
            continue;
        };
        let base = found.parent().unwrap_or(Path::new(""));
        let entries = names
            .iter()
            .filter_map(|(i, n)| n.strip_prefix(base).ok().map(|rel| (*i, rel.to_path_buf())))
            .filter(|(_, rel)| !rel.as_os_str().is_empty() && rel.components().all(|c| matches!(c, Component::Normal(_))))
            .collect();
        return Ok(Plan { zip: zip_path.to_path_buf(), game, title, marker, entries });
    }
    Err("Not recognised. Souls-like installs Seamless Co-op (DS1, DS2, DS3) and Honest Merchant zips.".into())
}

pub fn install(plan: &Plan, game_dir: &Path) -> Result<String, String> {
    let mut ar = open(&plan.zip)?;
    let backup_dir = game_dir.join(".souls-like-backup").join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
    let (mut written, mut replaced, mut kept) = (0, 0, 0);
    let ioe = |e: io::Error| e.to_string();
    for (idx, rel) in &plan.entries {
        let dest = game_dir.join(rel);
        if rel.to_string_lossy().to_lowercase().ends_with("_settings.ini") && dest.exists() {
            kept += 1;
            continue;
        }
        if dest.exists() {
            let saved = backup_dir.join(rel);
            fs::create_dir_all(saved.parent().unwrap()).map_err(ioe)?;
            fs::copy(&dest, &saved).map_err(|e| format!("Could not back up {}: {e}", rel.display()))?;
            replaced += 1;
        }
        fs::create_dir_all(dest.parent().unwrap()).map_err(ioe)?;
        let mut out = fs::File::create(&dest).map_err(|e| format!("Could not write {}: {e}", rel.display()))?;
        io::copy(&mut ar.by_index(*idx).map_err(|e| e.to_string())?, &mut out).map_err(ioe)?;
        written += 1;
    }
    let mut msg = format!("Installed {} for {}: {written} files", plan.title, GAMES[plan.game].name);
    if replaced > 0 {
        msg += &format!(", {replaced} replaced (backed up)");
    }
    if kept > 0 {
        msg += ", your settings file kept";
    }
    Ok(msg + ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (n, data) in files {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn detects_the_game_keeps_settings_and_rejects_unknown_zips() {
        let dir = std::env::temp_dir().join("souls-like-mod-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("game/SeamlessCoop")).unwrap();
        make_zip(&dir.join("m.zip"), &[("Pack/ds3sc_launcher.exe", b"exe"), ("Pack/SeamlessCoop/ds3sc.dll", b"dll"), ("Pack/SeamlessCoop/ds3sc_settings.ini", b"new")]);
        fs::write(dir.join("game/SeamlessCoop/ds3sc_settings.ini"), b"mine").unwrap();
        let plan = inspect(&dir.join("m.zip")).unwrap();
        assert_eq!((plan.game, plan.entries.len()), (2, 3));
        install(&plan, &dir.join("game")).unwrap();
        assert_eq!(fs::read(dir.join("game/SeamlessCoop/ds3sc_settings.ini")).unwrap(), b"mine");
        assert!(dir.join("game/ds3sc_launcher.exe").exists());
        make_zip(&dir.join("x.zip"), &[("readme.txt", b"hi")]);
        assert!(inspect(&dir.join("x.zip")).is_err());
    }
}
