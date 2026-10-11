use std::{fs, path::Path};

pub fn load(key: &str, exe: &Path) -> Option<image::RgbaImage> {
    let cache = dirs::cache_dir()?.join("souls-like/icons").join(format!("{key}.png"));
    if let Ok(img) = image::open(&cache) {
        return Some(img.to_rgba8());
    }
    let img = extract(exe)?;
    let _ = fs::create_dir_all(cache.parent()?);
    let _ = img.save(&cache);
    Some(img)
}

fn extract(exe: &Path) -> Option<image::RgbaImage> {
    let bytes = fs::read(exe).ok()?;
    let res = match pelite::PeFile::from_bytes(&bytes).ok()? {
        pelite::Wrap::T64(p) => pelite::pe64::Pe::resources(p).ok()?,
        pelite::Wrap::T32(p) => pelite::pe32::Pe::resources(p).ok()?,
    };
    res.icons()
        .flatten()
        .filter_map(|(_, group)| {
            let mut ico = vec![];
            group.write(&mut ico).ok()?;
            image::load_from_memory_with_format(&ico, image::ImageFormat::Ico).ok()
        })
        .map(|i| i.to_rgba8())
        .max_by_key(|i| i.width())
}
