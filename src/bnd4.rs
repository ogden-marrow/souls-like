//! Read the BND4 container used by Souls save files, and decrypt its AES-128-CBC entries.
//! Each encrypted entry is `[16-byte IV][ciphertext]`; the plaintext starts with a 16-byte
//! salt, a little-endian length, the data, then PKCS#7 padding.

use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};

pub struct Entry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn u64le(d: &[u8], o: usize) -> Option<u64> {
    d.get(o..o + 8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}

fn utf16_cstr(d: &[u8], o: usize) -> String {
    let units: Vec<u16> = d
        .get(o..)
        .unwrap_or_default()
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|u| *u != 0)
        .take(64)
        .collect();
    String::from_utf16_lossy(&units)
}

pub fn parse(d: &[u8]) -> Result<Vec<Entry>, String> {
    if d.get(..4) != Some(b"BND4") {
        return Err("Not a Souls save file (missing BND4 header).".into());
    }
    let count = u32le(d, 0x0c).ok_or("Truncated save file")? as usize;
    let esz = u64le(d, 0x20).ok_or("Truncated save file")? as usize;
    if esz < 0x20 || count > 4096 {
        return Err("Unexpected save file layout.".into());
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = 0x40 + i * esz;
        let size = u64le(d, o + 8).ok_or("Truncated save file")? as usize;
        let offset = u32le(d, o + 16).ok_or("Truncated save file")? as usize;
        let name_off = u32le(d, o + 20).ok_or("Truncated save file")? as usize;
        if offset
            .checked_add(size)
            .map(|e| e > d.len())
            .unwrap_or(true)
        {
            return Err("Save file entry is out of range.".into());
        }
        out.push(Entry {
            name: utf16_cstr(d, name_off),
            offset,
            size,
        });
    }
    Ok(out)
}

/// Decrypt an entry. Returns the plaintext (with the trailing padding removed), or `None` if the
/// entry isn't encrypted with this key (bad padding).
pub fn decrypt(d: &[u8], e: &Entry, key: &[u8; 16]) -> Option<Vec<u8>> {
    let blob = d.get(e.offset..e.offset + e.size)?;
    if blob.len() < 32 {
        return None;
    }
    let (iv, ct) = blob.split_at(16);
    let ct = &ct[..ct.len() / 16 * 16];
    let mut buf = ct.to_vec();
    cbc::Decryptor::<Aes128>::new(key.into(), iv.into())
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .ok()?;
    let pad = *buf.last()? as usize;
    if pad == 0
        || pad > 16
        || buf.len() < pad
        || buf[buf.len() - pad..].iter().any(|b| *b as usize != pad)
    {
        return None;
    }
    buf.truncate(buf.len() - pad);
    Some(buf)
}
