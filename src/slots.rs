use crate::{
    bnd4::{self, Entry},
    games::{Game, Slots, Summary},
};
use std::{fs, ops::Range, path::Path};

fn find<'a>(entries: &'a [Entry], name: &str) -> Result<&'a Entry, String> {
    entries.iter().find(|e| e.name == name).ok_or_else(|| format!("The save has no {name}."))
}

fn block(s: &Summary, slot: usize) -> Range<usize> {
    let start = s.first - s.lead + s.stride * slot;
    start..start + s.stride
}

fn name_at(plain: &[u8], off: usize) -> String {
    let units: Vec<u16> = plain.get(off..).unwrap_or_default().chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).take(16).collect();
    String::from_utf16_lossy(&units)
}

fn occupants_of(slots: &Slots, data: &[u8]) -> Result<Vec<Option<String>>, String> {
    let entries = bnd4::parse(data)?;
    let names = &slots.summaries[0];
    let plain = bnd4::decrypt(data, find(&entries, names.entry)?, &slots.key).ok_or("Could not decrypt the save; is this the right game?")?;
    Ok((0..slots.count).map(|i| Some(name_at(&plain, names.first + names.stride * i)).filter(|n| !n.is_empty())).collect())
}

pub fn occupants(game: &Game, save: &Path) -> Result<Vec<Option<String>>, String> {
    occupants_of(&game.slots, &fs::read(save).map_err(|e| e.to_string())?)
}

pub fn copy_slot(game: &Game, src: &Path, src_slot: usize, dst: &Path, dst_slot: usize) -> Result<(), String> {
    let src_data = fs::read(src).map_err(|e| format!("Could not read the source save: {e}"))?;
    let dst_data = if src == dst { src_data.clone() } else { fs::read(dst).map_err(|e| format!("Could not read the destination save: {e}"))? };
    let out = copy_slot_bytes(&game.slots, &src_data, src_slot, &dst_data, dst_slot)?;
    let tmp = dst.with_extension("souls-like-tmp");
    fs::write(&tmp, &out).map_err(|e| format!("Could not write the new save: {e}"))?;
    fs::rename(&tmp, dst).map_err(|e| format!("Could not replace the save: {e}"))
}

fn copy_slot_bytes(slots: &Slots, src_data: &[u8], src_slot: usize, dst_data: &[u8], dst_slot: usize) -> Result<Vec<u8>, String> {
    if src_slot >= slots.count || dst_slot >= slots.count {
        return Err("Slot number out of range.".into());
    }
    let (src_entries, dst_entries) = (bnd4::parse(src_data)?, bnd4::parse(dst_data)?);
    let moved_name = occupants_of(slots, src_data)?[src_slot].clone().ok_or("The source slot is empty.")?;
    let index_of = |name: &str| dst_entries.iter().position(|e| e.name == name).ok_or_else(|| format!("The save has no {name}."));
    let mut replace: Vec<(usize, Vec<u8>)> = vec![];

    for (from, to) in (slots.entries)(src_slot).iter().zip((slots.entries)(dst_slot)) {
        let (se, di) = (find(&src_entries, from)?, index_of(&to)?);
        if se.size != dst_entries[di].size {
            return Err("The two saves use different slot sizes.".into());
        }
        replace.push((di, bnd4::blob(src_data, se).to_vec()));
    }

    for s in slots.summaries {
        let (se, di) = (find(&src_entries, s.entry)?, index_of(s.entry)?);
        let src_sum = bnd4::decrypt(src_data, se, &slots.key).ok_or("Could not decrypt the source summary.")?;
        let mut dst_sum = bnd4::decrypt(dst_data, &dst_entries[di], &slots.key).ok_or("Could not decrypt the destination summary.")?;
        let (from, to) = (block(s, src_slot), block(s, dst_slot));
        if from.end > src_sum.len() || to.end > dst_sum.len() {
            return Err("Unexpected profile summary size.".into());
        }
        dst_sum[to].copy_from_slice(&src_sum[from]);
        if let Some(t) = s.table.filter(|t| t + src_slot.max(dst_slot) < src_sum.len().min(dst_sum.len())) {
            dst_sum[t + dst_slot] = src_sum[t + src_slot];
        }
        let blob = bnd4::encrypt(dst_data, &dst_entries[di], &dst_sum, &slots.key).filter(|b| b.len() == dst_entries[di].size);
        replace.push((di, blob.ok_or("Could not re-encrypt the summary.")?));
    }

    let mut out = dst_data.to_vec();
    for (di, blob) in &replace {
        let e = &dst_entries[*di];
        out[e.offset..e.offset + e.size].copy_from_slice(blob);
    }

    let out_entries = bnd4::parse(&out)?;
    let layout_kept = out.len() == dst_data.len() && out_entries.len() == dst_entries.len();
    let blobs_ok = out_entries.iter().zip(&dst_entries).enumerate().all(|(i, (a, b))| bnd4::blob(&out, a) == replace.iter().find(|(di, _)| *di == i).map_or(bnd4::blob(dst_data, b), |(_, blob)| blob));
    let summaries_ok = slots.summaries.iter().all(|s| {
        let read = |d: &[u8], es: &[Entry]| find(es, s.entry).ok().and_then(|e| bnd4::decrypt(d, e, &slots.key));
        matches!((read(&out, &out_entries), read(src_data, &src_entries)), (Some(a), Some(b)) if a[block(s, dst_slot)] == b[block(s, src_slot)])
    });
    let listed = occupants_of(slots, &out)?[dst_slot].as_ref() == Some(&moved_name);
    if layout_kept && blobs_ok && summaries_ok && listed { Ok(out) } else { Err("Verification failed; nothing was written.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::GAMES;
    use aes::Aes128;
    use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};

    fn encrypt(key: &[u8; 16], iv: u8, plain: &[u8]) -> Vec<u8> {
        let mut buf = plain.to_vec();
        buf.resize(plain.len() + 16 - plain.len() % 16, (16 - plain.len() % 16) as u8);
        let len = buf.len();
        cbc::Encryptor::<Aes128>::new(key.into(), (&[iv; 16]).into()).encrypt_padded_mut::<NoPadding>(&mut buf, len).unwrap();
        [[iv; 16].as_slice(), &buf].concat()
    }

    fn container(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        let table = 0x40 + entries.len() * 0x20;
        let (mut names, mut offsets) = (vec![], vec![]);
        for (n, _) in entries {
            offsets.push(table + names.len());
            names.extend(n.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
        }
        let data_start = (table + names.len()).div_ceil(16) * 16;
        let mut out = vec![0u8; data_start];
        out[..4].copy_from_slice(b"BND4");
        out[0x0c..0x10].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        out[0x20..0x28].copy_from_slice(&0x20u64.to_le_bytes());
        out[table..table + names.len()].copy_from_slice(&names);
        let mut off = data_start;
        for (i, (_, blob)) in entries.iter().enumerate() {
            let o = 0x40 + i * 0x20;
            out[o + 8..o + 16].copy_from_slice(&(blob.len() as u64).to_le_bytes());
            out[o + 16..o + 20].copy_from_slice(&(off as u32).to_le_bytes());
            out[o + 20..o + 24].copy_from_slice(&(offsets[i] as u32).to_le_bytes());
            off += blob.len();
        }
        entries.iter().fold(out, |acc, (_, blob)| [acc, blob.clone()].concat())
    }

    fn build(game: usize, chars: &[(usize, &str, u8)]) -> Vec<u8> {
        let slots = &GAMES[game].slots;
        let mut entries: Vec<(String, Vec<u8>)> = vec![];
        for i in 0..slots.count {
            let fill = chars.iter().find(|c| c.0 == i).map_or(0, |c| c.2);
            for (k, name) in (slots.entries)(i).into_iter().enumerate() {
                entries.push((name, encrypt(&slots.key, (i * 2 + k) as u8 + 1, &[vec![i as u8; 4], vec![fill; 300]].concat())));
            }
        }
        for (n, s) in slots.summaries.iter().enumerate() {
            let mut sum = vec![0u8; s.first - s.lead + s.stride * slots.count + 200];
            for (slot, name, fill) in chars {
                s.table.into_iter().for_each(|t| sum[t + slot] = 0x10 + *slot as u8);
                sum[block(s, *slot)].fill(*fill);
                let o = s.first + s.stride * slot;
                sum[o..o + name.len() * 2 + 2].copy_from_slice(&name.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect::<Vec<_>>());
            }
            entries.push((s.entry.to_string(), encrypt(&slots.key, 100 + n as u8, &sum)));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        container(&entries)
    }

    fn names(game: usize, data: &[u8]) -> Vec<(usize, String)> {
        occupants_of(&GAMES[game].slots, data).unwrap().into_iter().enumerate().filter_map(|(i, n)| Some((i, n?))).collect()
    }

    #[test]
    fn copying_a_character_moves_exactly_its_data_in_every_game() {
        for game in 0..3 {
            let slots = &GAMES[game].slots;
            let (a, b) = (build(game, &[(0, "Alpha", 0xA1), (2, "Gamma", 0xA3)]), build(game, &[(0, "Bravo", 0xB1)]));
            let out = copy_slot_bytes(slots, &a, 2, &b, 5).unwrap();
            assert_eq!(names(game, &out), [(0, "Bravo".to_string()), (5, "Gamma".to_string())]);

            let (ea, eb, eo) = (bnd4::parse(&a).unwrap(), bnd4::parse(&b).unwrap(), bnd4::parse(&out).unwrap());
            let (to, from) = ((slots.entries)(5), (slots.entries)(2));
            let sums: Vec<&str> = slots.summaries.iter().map(|s| s.entry).collect();
            for (x, y) in eo.iter().zip(&eb) {
                match to.iter().position(|n| *n == x.name) {
                    Some(k) => assert_eq!(bnd4::blob(&out, x), bnd4::blob(&a, find(&ea, &from[k]).unwrap())),
                    None if !sums.contains(&x.name.as_str()) => assert_eq!(bnd4::blob(&out, x), bnd4::blob(&b, y)),
                    None => {}
                }
            }
            for s in slots.summaries {
                let plain = |d: &[u8], es: &[Entry]| bnd4::decrypt(d, find(es, s.entry).unwrap(), &slots.key).unwrap();
                let (sa, sb, so) = (plain(&a, &ea), plain(&b, &eb), plain(&out, &eo));
                assert_eq!(so[block(s, 5)], sa[block(s, 2)]);
                assert!((0..so.len()).filter(|i| so[*i] != sb[*i]).all(|i| block(s, 5).contains(&i) || s.table.is_some_and(|t| i == t + 5)));
            }

            assert!(copy_slot_bytes(slots, &a, 4, &a, 6).is_err());
            assert_eq!(copy_slot_bytes(slots, &a, 0, &a, 0).unwrap(), a);

            let dir = std::env::temp_dir().join(format!("souls-like-slots-{game}"));
            fs::create_dir_all(&dir).unwrap();
            let (pa, pb) = (dir.join("a.co2"), dir.join("b.sl2"));
            fs::write(&pa, &a).unwrap();
            fs::write(&pb, &b).unwrap();
            copy_slot(&GAMES[game], &pa, 2, &pb, 3).unwrap();
            assert_eq!(names(game, &fs::read(&pb).unwrap()), [(0, "Bravo".to_string()), (3, "Gamma".to_string())]);
            assert!(!dir.join("b.souls-like-tmp").exists());
        }
    }
}
