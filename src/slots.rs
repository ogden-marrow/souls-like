//! Character slots inside a save file: who is in each slot, and copying a character between saves.
//!
//! A slot's encrypted data blocks are moved untouched; the "profile summary" tables the game uses for
//! its character list get the matching block (re-encrypted with their original IV). The result is
//! verified byte for byte before anything is written.

use crate::{
    bnd4::{self, Entry},
    games::{Game, NameSource, SlotLayout, Summary},
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

fn find<'a>(entries: &'a [Entry], name: &str) -> Result<&'a Entry, String> {
    entries
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("The save has no {name}."))
}

/// The used character slots of a save. `Ok(None)` when this game's layout isn't supported.
pub fn read(game: &Game, save: &Path) -> Result<Option<Vec<Slot>>, String> {
    let Some(layout) = &game.slots else {
        return Ok(None);
    };
    let data = fs::read(save).map_err(|e| e.to_string())?;
    read_bytes(layout, &data).map(Some)
}

pub(crate) fn read_bytes(layout: &SlotLayout, data: &[u8]) -> Result<Vec<Slot>, String> {
    let entries = bnd4::parse(data)?;
    let summary = match layout.names {
        NameSource::Summary { entry, .. } => {
            let e = find(&entries, entry)?;
            Some(
                bnd4::decrypt(data, e, &layout.key)
                    .ok_or_else(|| format!("Could not decrypt {entry}; is this the right game?"))?,
            )
        }
        NameSource::InSlot(_) => None,
    };
    let mut out = Vec::new();
    for i in 0..layout.count {
        match layout.names {
            NameSource::Summary { first, stride, .. } => {
                let name = summary
                    .as_deref()
                    .map(|s| name_at(s, first + stride * i))
                    .unwrap_or_default();
                if !name.is_empty() {
                    out.push(Slot { index: i, name });
                }
            }
            NameSource::InSlot(off) => {
                let Some(first_entry) = (layout.slot_entries)(i).into_iter().next() else {
                    continue;
                };
                let Some(e) = entries.iter().find(|e| e.name == first_entry) else {
                    continue;
                };
                let Some(plain) = bnd4::decrypt(data, e, &layout.key) else {
                    return Err(format!(
                        "Could not decrypt {first_entry}; is this the right game?"
                    ));
                };
                // an unused slot is all zeros after the 20-byte header
                if plain.iter().skip(20).filter(|b| **b != 0).count() > 64 {
                    let name = name_at(&plain, off);
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
        }
    }
    Ok(out)
}

/// Who is in each slot: one entry per slot, `None` when empty.
pub fn occupants(game: &Game, save: &Path) -> Result<Vec<Option<String>>, String> {
    let count = game.slots.as_ref().map(|l| l.count).unwrap_or(0);
    let mut v: Vec<Option<String>> = vec![None; count];
    for s in read(game, save)?.unwrap_or_default() {
        if s.index < count {
            v[s.index] = Some(s.name);
        }
    }
    Ok(v)
}

/// Whether copying single characters is supported for this game.
pub fn can_copy(game: &Game) -> bool {
    game.slots
        .as_ref()
        .map(|l| !l.summaries.is_empty())
        .unwrap_or(false)
}

/// Copy the character in `src_slot` of `src` over `dst_slot` of `dst` (they may be the same file).
/// The caller is expected to have backed up `dst`.
pub fn copy_slot(
    game: &Game,
    src: &Path,
    src_slot: usize,
    dst: &Path,
    dst_slot: usize,
) -> Result<(), String> {
    let layout = game
        .slots
        .as_ref()
        .filter(|l| !l.summaries.is_empty())
        .ok_or("Characters can't be copied in this game yet.")?;
    let src_data = fs::read(src).map_err(|e| format!("Could not read the source save: {e}"))?;
    let dst_data = if src == dst {
        src_data.clone()
    } else {
        fs::read(dst).map_err(|e| format!("Could not read the destination save: {e}"))?
    };
    let out = copy_slot_bytes(layout, &src_data, src_slot, &dst_data, dst_slot)?;
    // write next to the target, then rename, so a failure can't leave a half-written save
    let tmp = dst.with_extension("souls-like-tmp");
    fs::write(&tmp, &out).map_err(|e| format!("Could not write the new save: {e}"))?;
    fs::rename(&tmp, dst).map_err(|e| format!("Could not replace the save: {e}"))?;
    Ok(())
}

fn block(s: &Summary, slot: usize) -> std::ops::Range<usize> {
    let start = s.first - s.lead + s.stride * slot;
    start..start + s.stride
}

pub(crate) fn copy_slot_bytes(
    layout: &SlotLayout,
    src_data: &[u8],
    src_slot: usize,
    dst_data: &[u8],
    dst_slot: usize,
) -> Result<Vec<u8>, String> {
    if layout.summaries.is_empty() {
        return Err("Characters can't be copied in this game yet.".into());
    }
    if src_slot >= layout.count || dst_slot >= layout.count {
        return Err("Slot number out of range.".into());
    }
    let src_entries = bnd4::parse(src_data)?;
    let dst_entries = bnd4::parse(dst_data)?;

    // the source slot must be a real character
    if !read_bytes(layout, src_data)?
        .iter()
        .any(|s| s.index == src_slot)
    {
        return Err("The source slot is empty.".into());
    }

    // 1. the slot's own entries, moved as raw encrypted blocks
    let mut replace: Vec<(usize, Vec<u8>)> = Vec::new(); // (dst entry index, new blob)
    let (src_names, dst_names) = (
        (layout.slot_entries)(src_slot),
        (layout.slot_entries)(dst_slot),
    );
    for (sn, dn) in src_names.iter().zip(&dst_names) {
        let se = find(&src_entries, sn)?;
        let di = dst_entries
            .iter()
            .position(|e| &e.name == dn)
            .ok_or_else(|| format!("The save has no {dn}."))?;
        if se.size != dst_entries[di].size {
            return Err("The two saves use different slot sizes.".into());
        }
        replace.push((di, bnd4::blob(src_data, se).to_vec()));
    }

    // 2. the profile summaries: move the slot's block, re-encrypt with the original IV
    for s in layout.summaries {
        let se = find(&src_entries, s.entry)?;
        let di = dst_entries
            .iter()
            .position(|e| e.name == s.entry)
            .ok_or_else(|| format!("The save has no {}.", s.entry))?;
        let de = &dst_entries[di];
        let src_sum = bnd4::decrypt(src_data, se, &layout.key)
            .ok_or("Could not decrypt the source summary.")?;
        let mut dst_sum = bnd4::decrypt(dst_data, de, &layout.key)
            .ok_or("Could not decrypt the destination summary.")?;
        let (sb, db) = (block(s, src_slot), block(s, dst_slot));
        if sb.end > src_sum.len() || db.end > dst_sum.len() {
            return Err("Unexpected profile summary size.".into());
        }
        dst_sum[db].copy_from_slice(&src_sum[sb]);
        if let Some(t) = s.table {
            if t + src_slot < src_sum.len() && t + dst_slot < dst_sum.len() {
                dst_sum[t + dst_slot] = src_sum[t + src_slot];
            }
        }
        let blob = bnd4::encrypt(dst_data, de, &dst_sum, &layout.key)
            .ok_or("Could not re-encrypt the summary.")?;
        if blob.len() != de.size {
            return Err("Re-encrypted summary changed size.".into());
        }
        replace.push((di, blob));
    }

    let mut out = dst_data.to_vec();
    for (di, blob) in &replace {
        let e = &dst_entries[*di];
        out[e.offset..e.offset + e.size].copy_from_slice(blob);
    }

    // ---- verify the result before anyone writes it
    let out_entries = bnd4::parse(&out)?;
    if out.len() != dst_data.len() || out_entries.len() != dst_entries.len() {
        return Err("Verification failed: file layout changed.".into());
    }
    for (i, (a, b)) in out_entries.iter().zip(&dst_entries).enumerate() {
        let expected: &[u8] = match replace.iter().find(|(di, _)| *di == i) {
            Some((_, blob)) => blob,
            None => bnd4::blob(dst_data, b),
        };
        if bnd4::blob(&out, a) != expected {
            return Err(format!("Verification failed on {}.", a.name));
        }
    }
    for s in layout.summaries {
        let a = bnd4::decrypt(&out, find(&out_entries, s.entry)?, &layout.key)
            .ok_or("Verification failed: summary unreadable.")?;
        let b = bnd4::decrypt(src_data, find(&src_entries, s.entry)?, &layout.key)
            .ok_or("Verification failed: summary unreadable.")?;
        if a[block(s, dst_slot)] != b[block(s, src_slot)] {
            return Err("Verification failed: summary block mismatch.".into());
        }
    }
    let after = read_bytes(layout, &out)?;
    let moved = read_bytes(layout, src_data)?
        .into_iter()
        .find(|s| s.index == src_slot)
        .map(|s| s.name);
    if after.iter().find(|s| s.index == dst_slot).map(|s| &s.name) != moved.as_ref() {
        return Err(
            "Verification failed: the character list doesn't show the copied character.".into(),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GAMES;
    use aes::Aes128;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};

    fn enc(key: &[u8; 16], iv: [u8; 16], plain: &[u8]) -> Vec<u8> {
        let pad = 16 - plain.len() % 16;
        let mut buf = plain.to_vec();
        buf.extend(std::iter::repeat_n(pad as u8, pad));
        let len = buf.len();
        cbc::Encryptor::<Aes128>::new(key.into(), (&iv).into())
            .encrypt_padded_mut::<NoPadding>(&mut buf, len)
            .unwrap();
        let mut out = iv.to_vec();
        out.extend(buf);
        out
    }

    fn container(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        let table = 0x40 + entries.len() * 0x20;
        let mut names = Vec::new();
        let mut name_offs = Vec::new();
        for (n, _) in entries {
            name_offs.push(table + names.len());
            for u in n.encode_utf16().chain([0u16]) {
                names.extend_from_slice(&u.to_le_bytes());
            }
        }
        let data_start = (table + names.len()).div_ceil(16) * 16;
        let mut out = vec![0u8; data_start];
        out[..4].copy_from_slice(b"BND4");
        out[0x0c..0x10].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        out[0x20..0x28].copy_from_slice(&0x20u64.to_le_bytes());
        out[0x28..0x30].copy_from_slice(&(data_start as u64).to_le_bytes());
        out[table..table + names.len()].copy_from_slice(&names);
        let mut off = data_start;
        for (i, (_, blob)) in entries.iter().enumerate() {
            let o = 0x40 + i * 0x20;
            out[o..o + 4].copy_from_slice(&0x50u32.to_le_bytes());
            out[o + 4..o + 8].copy_from_slice(&(-1i32).to_le_bytes());
            out[o + 8..o + 16].copy_from_slice(&(blob.len() as u64).to_le_bytes());
            out[o + 16..o + 20].copy_from_slice(&(off as u32).to_le_bytes());
            out[o + 20..o + 24].copy_from_slice(&(name_offs[i] as u32).to_le_bytes());
            off += blob.len();
        }
        for (_, blob) in entries {
            out.extend_from_slice(blob);
        }
        out
    }

    /// A synthetic save for a game with summary tables: every slot entry, every summary, plus filler.
    /// `chars` = (slot, name, fill byte).
    fn build(game: usize, chars: &[(usize, &str, u8)]) -> Vec<u8> {
        let layout = GAMES[game].slots.as_ref().unwrap();
        let key = layout.key;
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut serial = 1u8;
        for i in 0..layout.count {
            for name in (layout.slot_entries)(i) {
                let fill = chars.iter().find(|c| c.0 == i).map(|c| c.2).unwrap_or(0);
                let mut plain = vec![i as u8, 7, 7, 7];
                plain.extend(std::iter::repeat_n(fill, 300));
                entries.push((name, enc(&key, [serial; 16], &plain)));
                serial += 1;
            }
        }
        for s in layout.summaries {
            let mut sum = vec![0u8; s.first - s.lead + s.stride * layout.count + 200];
            for (i, v) in sum.iter_mut().enumerate().take(16) {
                *v = i as u8 + 1;
            }
            for (slot, name, fill) in chars {
                if let Some(t) = s.table {
                    sum[t + slot] = 0x10 + *slot as u8;
                }
                for b in &mut sum[block(s, *slot)] {
                    *b = *fill;
                }
                let o = s.first + s.stride * slot;
                for (j, u) in name.encode_utf16().enumerate() {
                    sum[o + j * 2..o + j * 2 + 2].copy_from_slice(&u.to_le_bytes());
                }
                let end = o + name.len() * 2;
                sum[end..end + 2].copy_from_slice(&[0, 0]);
            }
            entries.push((s.entry.to_string(), enc(&key, [serial; 16], &sum)));
            serial += 1;
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        container(&entries)
    }

    fn names(game: usize, data: &[u8]) -> Vec<(usize, String)> {
        read_bytes(GAMES[game].slots.as_ref().unwrap(), data)
            .unwrap()
            .into_iter()
            .map(|s| (s.index, s.name))
            .collect()
    }

    /// The bit-for-bit properties a character copy must have, for any supported game.
    fn check_copy(game: usize) {
        let layout = GAMES[game].slots.as_ref().unwrap();
        let a = build(game, &[(0, "Alpha", 0xA1), (2, "Gamma", 0xA3)]);
        let b = build(game, &[(0, "Bravo", 0xB1)]);
        assert_eq!(names(game, &b), vec![(0, "Bravo".to_string())]);
        let out = copy_slot_bytes(layout, &a, 2, &b, 5).unwrap();
        assert_eq!(out.len(), b.len());
        assert_eq!(
            names(game, &out),
            vec![(0, "Bravo".to_string()), (5, "Gamma".to_string())]
        );

        let (ea, eb, eo) = (
            bnd4::parse(&a).unwrap(),
            bnd4::parse(&b).unwrap(),
            bnd4::parse(&out).unwrap(),
        );
        let moved: Vec<String> = (layout.slot_entries)(5);
        let src_moved: Vec<String> = (layout.slot_entries)(2);
        let summary_entries: Vec<&str> = layout.summaries.iter().map(|s| s.entry).collect();
        for (x, y) in eo.iter().zip(&eb) {
            if let Some(k) = moved.iter().position(|n| *n == x.name) {
                // the moved character is byte-identical to the source slot's block
                let se = ea.iter().find(|e| e.name == src_moved[k]).unwrap();
                assert_eq!(bnd4::blob(&out, x), bnd4::blob(&a, se), "{}", x.name);
            } else if !summary_entries.contains(&x.name.as_str()) {
                assert_eq!(
                    bnd4::blob(&out, x),
                    bnd4::blob(&b, y),
                    "untouched {}",
                    x.name
                );
            }
        }
        for s in layout.summaries {
            let get = |d: &[u8], es: &[Entry]| {
                bnd4::decrypt(
                    d,
                    es.iter().find(|e| e.name == s.entry).unwrap(),
                    &layout.key,
                )
                .unwrap()
            };
            let (sa, sb, so) = (get(&a, &ea), get(&b, &eb), get(&out, &eo));
            assert_eq!(so[block(s, 5)], sa[block(s, 2)]);
            assert_eq!(so[block(s, 0)], sb[block(s, 0)]);
            let changed: Vec<usize> = (0..so.len()).filter(|i| so[*i] != sb[*i]).collect();
            let table_hit = |i: usize| s.table.map(|t| i == t + 5).unwrap_or(false);
            assert!(
                changed
                    .iter()
                    .all(|i| block(s, 5).contains(i) || table_hit(*i))
            );
        }
    }

    #[test]
    fn ds1_copy_moves_character_bit_for_bit() {
        check_copy(0);
    }

    #[test]
    fn ds3_copy_moves_character_bit_for_bit() {
        check_copy(2);
    }

    #[test]
    fn ds2_copy_moves_character_bit_for_bit() {
        check_copy(1);
    }

    #[test]
    fn copy_rejects_empty_source_and_copy_onto_self_is_stable() {
        for game in [0, 1, 2] {
            let layout = GAMES[game].slots.as_ref().unwrap();
            let a = build(game, &[(1, "Alpha", 0xA1)]);
            assert!(copy_slot_bytes(layout, &a, 4, &a, 6).is_err());
            assert_eq!(copy_slot_bytes(layout, &a, 1, &a, 1).unwrap(), a);
        }
    }

    #[test]
    fn copy_slot_on_disk_writes_a_valid_save() {
        for game in [0usize, 1, 2] {
            let dir = std::env::temp_dir().join(format!("souls-like-copy-disk-{game}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let (a, b) = (dir.join("a.co2"), dir.join("b.sl2"));
            std::fs::write(&a, build(game, &[(0, "Alpha", 0xA1), (1, "Beta", 0xA2)])).unwrap();
            std::fs::write(&b, build(game, &[(0, "Bravo", 0xB1)])).unwrap();
            copy_slot(&GAMES[game], &a, 1, &b, 2).unwrap();
            let got: Vec<_> = read(&GAMES[game], &b)
                .unwrap()
                .unwrap()
                .into_iter()
                .map(|s| (s.index, s.name))
                .collect();
            assert_eq!(got, vec![(0, "Bravo".to_string()), (2, "Beta".to_string())]);
            assert!(!dir.join("b.souls-like-tmp").exists());
            copy_slot(&GAMES[game], &b, 0, &b, 6).unwrap();
            let got: Vec<_> = read(&GAMES[game], &b)
                .unwrap()
                .unwrap()
                .into_iter()
                .map(|s| s.index)
                .collect();
            assert_eq!(got, vec![0, 2, 6]);
        }
    }

    /// Lists characters in the installed games' saves: `cargo test slots_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn slots_live() {
        for g in &GAMES {
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
                    read(g, &s.path).map(|o| o.map(|v| v
                        .iter()
                        .map(|s| (s.index, s.name.clone()))
                        .collect::<Vec<_>>()))
                );
            }
        }
    }

    /// The same copy checks on copies of your real saves: `cargo test copy_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn copy_real() {
        for (gi, a_ext, a_slot, b_ext, b_slot) in [
            (2usize, "co2", 1usize, "sl2", 4usize),
            (1, "co2", 1, "sl2", 5),
            (0, "co2", 0, "sl2", 3),
        ] {
            let g = &GAMES[gi];
            let layout = g.slots.as_ref().unwrap();
            let loc = crate::platform::locate(g, "").unwrap();
            let root = crate::saves::find_root(
                g,
                &crate::platform::save_base_dir(g, Some(&loc), "").unwrap(),
            )
            .unwrap();
            let file =
                |ext: &str| std::fs::read(root.join(format!("{}.{ext}", g.save_file))).unwrap();
            let (a, b) = (file(a_ext), file(b_ext));
            let out = copy_slot_bytes(layout, &a, a_slot, &b, b_slot).unwrap();
            println!("{}: before {:?}", g.name, names_of(layout, &b));
            println!("{}: after  {:?}", g.name, names_of(layout, &out));
            // copying a character onto itself must be a no-op, byte for byte
            assert_eq!(copy_slot_bytes(layout, &a, a_slot, &a, a_slot).unwrap(), a);
        }
    }

    fn names_of(layout: &SlotLayout, d: &[u8]) -> Vec<(usize, String)> {
        read_bytes(layout, d)
            .unwrap()
            .into_iter()
            .map(|s| (s.index, s.name))
            .collect()
    }
}
