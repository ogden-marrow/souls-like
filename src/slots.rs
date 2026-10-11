//! Character slots inside a save file: which are used, and the character's name.

use crate::{
    bnd4,
    games::{Game, NameSource},
};
use std::{fs, path::Path};

pub struct Slot {
    pub index: usize,
    pub name: String,
}

fn name_at(plain: &[u8], off: usize) -> String {
    let units: Vec<u16> = plain
        .get(off..)
        .unwrap_or_default()
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|u| *u != 0)
        .take(16)
        .collect();
    String::from_utf16_lossy(&units)
}

/// The used character slots of a save. `Ok(None)` when this game's layout isn't supported yet.
pub fn read(game: &Game, save: &Path) -> Result<Option<Vec<Slot>>, String> {
    let Some(layout) = &game.slots else {
        return Ok(None);
    };
    let data = fs::read(save).map_err(|e| e.to_string())?;
    let entries = bnd4::parse(&data)?;
    let summary = match layout.names {
        NameSource::Summary { .. } => entries
            .iter()
            .find(|e| e.name == "USER_DATA010")
            .and_then(|e| bnd4::decrypt(&data, e, &layout.key)),
        NameSource::InSlot(_) => None,
    };
    let mut out = Vec::new();
    for i in 0..layout.count {
        let prefix = format!("USER_DATA{i:03}");
        let Some(e) = entries.iter().find(|e| e.name == prefix) else {
            continue;
        };
        let Some(plain) = bnd4::decrypt(&data, e, &layout.key) else {
            return Err(format!(
                "Could not decrypt {prefix}; is this the right game?"
            ));
        };
        // an unused slot is all zeros after the 20-byte header
        let used = plain.iter().skip(20).filter(|b| **b != 0).count() > 64;
        if used {
            let name = match layout.names {
                NameSource::InSlot(off) => name_at(&plain, off),
                NameSource::Summary { first, stride } => summary
                    .as_deref()
                    .map(|s| name_at(s, first + stride * i))
                    .unwrap_or_default(),
            };
            out.push(Slot {
                index: i,
                name: if name.is_empty() {
                    "(unnamed)".into()
                } else {
                    name
                },
            });
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    /// `cargo test slots_live -- --ignored --nocapture` lists characters in the installed games' saves.
    #[test]
    #[ignore]
    fn slots_live() {
        for g in &crate::games::GAMES {
            let loc = crate::platform::locate(g, "");
            let base = crate::platform::save_base_dir(g, loc.as_ref(), "");
            let Some(root) = base.and_then(|b| crate::saves::find_root(g, &b)) else {
                continue;
            };
            for s in crate::saves::list(g, &root) {
                println!(
                    "{} .{}: {:?}",
                    g.name,
                    s.ext,
                    super::read(g, &s.path).map(|o| o.map(|v| v
                        .iter()
                        .map(|s| (s.index, s.name.clone()))
                        .collect::<Vec<_>>()))
                );
            }
        }
    }
}
