//! Game icons, read straight out of each game's installed executable (nothing is bundled or
//! redistributed). The decoded icon is cached in the user's cache folder.

use std::{fs, path::Path};

pub fn load(key: &str, exe: &Path) -> Option<image::RgbaImage> {
    let cache = dirs::cache_dir()?
        .join("souls-like")
        .join("icons")
        .join(format!("{key}.png"));
    if let Ok(img) = image::open(&cache) {
        return Some(img.to_rgba8());
    }
    let img = extract(exe)?;
    if let Some(d) = cache.parent() {
        let _ = fs::create_dir_all(d);
    }
    let _ = img.save(&cache);
    Some(img)
}

/// The largest icon in the executable's resources.
pub fn extract(exe: &Path) -> Option<image::RgbaImage> {
    let bytes = fs::read(exe).ok()?;
    let pe = pelite::PeFile::from_bytes(&bytes).ok()?;
    let res = match pe {
        pelite::Wrap::T64(p) => pelite::pe64::Pe::resources(p).ok()?,
        pelite::Wrap::T32(p) => pelite::pe32::Pe::resources(p).ok()?,
    };
    let mut best: Option<image::RgbaImage> = None;
    for (_, group) in res.icons().flatten() {
        let mut ico = Vec::new();
        if group.write(&mut ico).is_err() {
            continue;
        }
        let Ok(img) = image::load_from_memory_with_format(&ico, image::ImageFormat::Ico) else {
            continue;
        };
        let img = img.to_rgba8();
        if best
            .as_ref()
            .map(|b| img.width() > b.width())
            .unwrap_or(true)
        {
            best = Some(img);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    /// `cargo test game_icons -- --ignored --nocapture` prints what is found in the installed games.
    #[test]
    #[ignore]
    fn game_icons() {
        for g in &crate::games::GAMES {
            if let Some(loc) = crate::platform::locate(g, "") {
                let img = super::extract(&loc.game_dir.join(g.proc_name));
                println!("{}: {:?}", g.name, img.map(|i| (i.width(), i.height())));
            }
        }
    }
}
