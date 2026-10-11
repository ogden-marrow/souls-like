//! Character slots inside a save file: which are used, and the character's name.

use crate::{
    bnd4,
    games::{Game, NameSource},
};
use std::{fs, path::Path};

/// Offset of the per-slot byte table at the start of the DS3 profile summary.
const SUMMARY_TABLE: usize = 0x20;

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

/// Whether copying characters is supported for this game.
pub fn can_copy(game: &Game) -> bool {
    matches!(
        game.slots.as_ref().map(|l| l.names),
        Some(NameSource::Summary { .. })
    )
}

/// Copy the character in `src_slot` of `src` over `dst_slot` of `dst` (they may be the same file).
/// The slot's encrypted block is moved untouched; the destination's profile summary (the character
/// list shown in the game menu) gets the matching block. Before writing, the result is re-read and
/// checked bit for bit. The caller is expected to have backed up `dst`.
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
        .ok_or("Characters can't be copied in this game yet.")?;
    let NameSource::Summary { first, stride } = layout.names else {
        return Err("Characters can't be copied in this game yet.".into());
    };
    if src_slot >= layout.count || dst_slot >= layout.count {
        return Err("Slot number out of range.".into());
    }
    let src_data = fs::read(src).map_err(|e| format!("Could not read the source save: {e}"))?;
    let dst_data = if src == dst {
        src_data.clone()
    } else {
        fs::read(dst).map_err(|e| format!("Could not read the destination save: {e}"))?
    };
    let out = copy_slot_bytes(
        layout, first, stride, &src_data, src_slot, &dst_data, dst_slot,
    )?;
    // write next to the target, then rename, so a failure can't leave a half-written save
    let tmp = dst.with_extension("souls-like-tmp");
    fs::write(&tmp, &out).map_err(|e| format!("Could not write the new save: {e}"))?;
    fs::rename(&tmp, dst).map_err(|e| format!("Could not replace the save: {e}"))?;
    Ok(())
}

fn find<'a>(entries: &'a [bnd4::Entry], name: &str) -> Result<&'a bnd4::Entry, String> {
    entries
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("The save has no {name}."))
}

pub(crate) fn copy_slot_bytes(
    layout: &crate::games::SlotLayout,
    first: usize,
    stride: usize,
    src_data: &[u8],
    src_slot: usize,
    dst_data: &[u8],
    dst_slot: usize,
) -> Result<Vec<u8>, String> {
    let src_entries = bnd4::parse(src_data)?;
    let dst_entries = bnd4::parse(dst_data)?;
    let slot_name = |i: usize| format!("USER_DATA{i:03}");
    let src_e = find(&src_entries, &slot_name(src_slot))?;
    let dst_e = find(&dst_entries, &slot_name(dst_slot))?;
    let src_sum_e = find(&src_entries, "USER_DATA010")?;
    let dst_sum_e = find(&dst_entries, "USER_DATA010")?;
    if src_e.size != dst_e.size {
        return Err("The two saves use different slot sizes.".into());
    }
    // the source slot must be a real character
    let src_plain = bnd4::decrypt(src_data, src_e, &layout.key)
        .ok_or("Could not decrypt the source character.")?;
    if src_plain.iter().skip(20).filter(|b| **b != 0).count() <= 64 {
        return Err("The source slot is empty.".into());
    }
    let src_sum = bnd4::decrypt(src_data, src_sum_e, &layout.key)
        .ok_or("Could not decrypt the source summary.")?;
    let mut dst_sum = bnd4::decrypt(dst_data, dst_sum_e, &layout.key)
        .ok_or("Could not decrypt the destination summary.")?;

    // block of the profile summary that describes one slot starts 0x20 before its name
    let block = |i: usize| {
        let start = first - 0x20 + stride * i;
        start..start + stride
    };
    let (sb, db) = (block(src_slot), block(dst_slot));
    if sb.end > src_sum.len() || db.end > dst_sum.len() {
        return Err("Unexpected profile summary size.".into());
    }
    dst_sum[db.clone()].copy_from_slice(&src_sum[sb.clone()]);
    if SUMMARY_TABLE + src_slot < src_sum.len() && SUMMARY_TABLE + dst_slot < dst_sum.len() {
        dst_sum[SUMMARY_TABLE + dst_slot] = src_sum[SUMMARY_TABLE + src_slot];
    }
    let new_sum_blob = bnd4::encrypt(dst_data, dst_sum_e, &dst_sum, &layout.key)
        .ok_or("Could not re-encrypt the summary.")?;
    if new_sum_blob.len() != dst_sum_e.size {
        return Err("Re-encrypted summary changed size.".into());
    }

    let mut out = dst_data.to_vec();
    out[dst_e.offset..dst_e.offset + dst_e.size].copy_from_slice(bnd4::blob(src_data, src_e));
    out[dst_sum_e.offset..dst_sum_e.offset + dst_sum_e.size].copy_from_slice(&new_sum_blob);

    // ---- verify the result before anyone writes it
    let out_entries = bnd4::parse(&out)?;
    if out.len() != dst_data.len() || out_entries.len() != dst_entries.len() {
        return Err("Verification failed: file layout changed.".into());
    }
    for (a, b) in out_entries.iter().zip(dst_entries.iter()) {
        let expected: &[u8] = if a.name == slot_name(dst_slot) {
            bnd4::blob(src_data, src_e)
        } else if a.name == "USER_DATA010" {
            &new_sum_blob
        } else {
            bnd4::blob(dst_data, b)
        };
        if bnd4::blob(&out, a) != expected {
            return Err(format!("Verification failed on {}.", a.name));
        }
    }
    let check_sum = bnd4::decrypt(&out, find(&out_entries, "USER_DATA010")?, &layout.key)
        .ok_or("Verification failed: summary unreadable.")?;
    if check_sum[block(dst_slot)] != src_sum[block(src_slot)] {
        return Err("Verification failed: summary block mismatch.".into());
    }
    let moved = bnd4::decrypt(&out, find(&out_entries, &slot_name(dst_slot))?, &layout.key)
        .ok_or("Verification failed: character unreadable.")?;
    if moved != src_plain {
        return Err("Verification failed: character data differs.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GAMES;
    use aes::Aes128;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
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

    const FIRST: usize = 4274;
    const STRIDE: usize = 554;

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

    /// A synthetic DS3-style save: ten slots plus the profile summary. `chars` = (slot, name, fill byte).
    fn build(chars: &[(usize, &str, u8)]) -> Vec<u8> {
        let key = GAMES[2].slots.as_ref().unwrap().key;
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..10 {
            let mut plain = vec![0u8; 16];
            plain[..4].copy_from_slice(&[7, 7, i as u8, 7]);
            plain.extend_from_slice(&300u32.to_le_bytes());
            let fill = chars.iter().find(|c| c.0 == i).map(|c| c.2).unwrap_or(0);
            plain.extend(std::iter::repeat_n(fill, 300));
            entries.push((
                format!("USER_DATA{i:03}"),
                enc(&key, [i as u8 + 1; 16], &plain),
            ));
        }
        let mut sum = vec![0u8; FIRST + STRIDE * 10 + 200];
        for (i, v) in sum.iter_mut().enumerate().take(16) {
            *v = i as u8;
        }
        for (slot, name, fill) in chars {
            sum[SUMMARY_TABLE + slot] = 0x10 + *slot as u8;
            let start = FIRST - 0x20 + STRIDE * slot;
            for b in &mut sum[start..start + STRIDE] {
                *b = *fill;
            }
            for (j, u) in name.encode_utf16().enumerate() {
                let o = FIRST + STRIDE * slot + j * 2;
                sum[o..o + 2].copy_from_slice(&u.to_le_bytes());
            }
            let end = FIRST + STRIDE * slot + name.len() * 2;
            sum[end..end + 2].copy_from_slice(&[0, 0]);
        }
        entries.push(("USER_DATA010".into(), enc(&key, [99; 16], &sum)));

        let table = 0x40 + entries.len() * 0x20;
        let mut names = Vec::new();
        let mut name_offs = Vec::new();
        for (n, _) in &entries {
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
        for (_, blob) in &entries {
            out.extend_from_slice(blob);
        }
        out
    }

    fn names(data: &[u8]) -> Vec<(usize, String)> {
        let dir = std::env::temp_dir().join(format!(
            "souls-like-slots-{}",
            data.len() ^ data.iter().map(|b| *b as usize).sum::<usize>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("DS30000.sl2");
        std::fs::write(&p, data).unwrap();
        read(&GAMES[2], &p)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|s| (s.index, s.name))
            .collect()
    }

    #[test]
    fn copy_moves_character_bit_for_bit() {
        let layout = GAMES[2].slots.as_ref().unwrap();
        let a = build(&[(0, "Alpha", 0xA1), (2, "Gamma", 0xA3)]);
        let b = build(&[(0, "Bravo", 0xB1)]);
        assert_eq!(names(&b), vec![(0, "Bravo".to_string())]);
        let out = copy_slot_bytes(layout, FIRST, STRIDE, &a, 2, &b, 5).unwrap();
        assert_eq!(out.len(), b.len());
        assert_eq!(
            names(&out),
            vec![(0, "Bravo".to_string()), (5, "Gamma".to_string())]
        );

        let ea = bnd4::parse(&a).unwrap();
        let eb = bnd4::parse(&b).unwrap();
        let eo = bnd4::parse(&out).unwrap();
        for ((x, y), z) in eo.iter().zip(&eb).zip(&ea) {
            if x.name == "USER_DATA005" {
                // the moved character is byte-identical to the source slot
                assert_eq!(bnd4::blob(&out, x), bnd4::blob(&a, &ea[2]));
            } else if x.name != "USER_DATA010" {
                // everything else is untouched
                assert_eq!(bnd4::blob(&out, x), bnd4::blob(&b, y), "{}", x.name);
            }
            let _ = z;
        }
        // summary: the block for slot 5 equals the source's block for slot 2; slot 0's block is unchanged
        let sa = bnd4::decrypt(&a, &ea[10], &layout.key).unwrap();
        let sb = bnd4::decrypt(&b, &eb[10], &layout.key).unwrap();
        let so = bnd4::decrypt(&out, &eo[10], &layout.key).unwrap();
        let blk = |i: usize| (FIRST - 0x20 + STRIDE * i)..(FIRST - 0x20 + STRIDE * (i + 1));
        assert_eq!(so[blk(5)], sa[blk(2)]);
        assert_eq!(so[blk(0)], sb[blk(0)]);
        assert_eq!(so[SUMMARY_TABLE + 5], sa[SUMMARY_TABLE + 2]);
        assert_eq!(so[SUMMARY_TABLE], sb[SUMMARY_TABLE]);
        // nothing outside those two places changed
        let changed: Vec<usize> = (0..so.len()).filter(|i| so[*i] != sb[*i]).collect();
        assert!(
            changed
                .iter()
                .all(|i| blk(5).contains(i) || *i == SUMMARY_TABLE + 5)
        );
    }

    #[test]
    fn copy_rejects_empty_source_and_copy_onto_self_is_stable() {
        let layout = GAMES[2].slots.as_ref().unwrap();
        let a = build(&[(1, "Alpha", 0xA1)]);
        assert!(copy_slot_bytes(layout, FIRST, STRIDE, &a, 4, &a, 6).is_err());
        let same = copy_slot_bytes(layout, FIRST, STRIDE, &a, 1, &a, 1).unwrap();
        assert_eq!(same, a);
    }

    #[test]
    fn copy_slot_on_disk_writes_a_valid_save() {
        let dir = std::env::temp_dir().join("souls-like-copy-disk");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("DS30000.co2"), dir.join("DS30000.sl2"));
        std::fs::write(&a, build(&[(0, "Alpha", 0xA1), (1, "Beta", 0xA2)])).unwrap();
        std::fs::write(&b, build(&[(0, "Bravo", 0xB1)])).unwrap();
        copy_slot(&GAMES[2], &a, 1, &b, 2).unwrap();
        let got: Vec<_> = read(&GAMES[2], &b)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|s| (s.index, s.name))
            .collect();
        assert_eq!(got, vec![(0, "Bravo".to_string()), (2, "Beta".to_string())]);
        assert!(!dir.join("DS30000.souls-like-tmp").exists());
        // copying onto a different slot of the same file works too
        copy_slot(&GAMES[2], &b, 0, &b, 6).unwrap();
        let got: Vec<_> = read(&GAMES[2], &b)
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|s| s.index)
            .collect();
        assert_eq!(got, vec![0, 2, 6]);
    }

    /// Runs the same checks on copies of your real saves:
    /// `cargo test copy_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn copy_real() {
        let g = &GAMES[2];
        let layout = g.slots.as_ref().unwrap();
        let loc = crate::platform::locate(g, "").unwrap();
        let root = crate::saves::find_root(
            g,
            &crate::platform::save_base_dir(g, Some(&loc), "").unwrap(),
        )
        .unwrap();
        let co2 = std::fs::read(root.join("DS30000.co2")).unwrap();
        let sl2 = std::fs::read(root.join("DS30000.sl2")).unwrap();
        // co2 slot 1 -> sl2 slot 4, then co2 slot 0 -> co2 slot 7
        for (src, ss, dst, ds) in [(&co2, 1, &sl2, 4), (&co2, 0, &co2, 7)] {
            let out = copy_slot_bytes(layout, FIRST, STRIDE, src, ss, dst, ds).unwrap();
            let es = bnd4::parse(src).unwrap();
            let eo = bnd4::parse(&out).unwrap();
            assert_eq!(bnd4::blob(&out, &eo[ds]), bnd4::blob(src, &es[ss]));
            println!("copied slot {ss} -> {ds}: {:?}", names(&out));
        }
    }
}
