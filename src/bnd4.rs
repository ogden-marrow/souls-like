use aes::Aes128;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::NoPadding};

pub struct Entry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

fn num(d: &[u8], o: usize, len: usize) -> Option<usize> {
    let mut b = [0u8; 8];
    b[..len].copy_from_slice(d.get(o..o + len)?);
    Some(u64::from_le_bytes(b) as usize)
}

fn utf16(d: &[u8], o: usize) -> String {
    let units: Vec<u16> = d.get(o..).unwrap_or_default().chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).take(64).collect();
    String::from_utf16_lossy(&units)
}

pub fn parse(d: &[u8]) -> Result<Vec<Entry>, String> {
    let bad = || "Not a readable Souls save file.".to_string();
    let (count, stride) = (num(d, 0x0c, 4).ok_or_else(bad)?, num(d, 0x20, 8).ok_or_else(bad)?);
    if d.get(..4) != Some(b"BND4") || stride < 0x20 || count > 4096 {
        return Err(bad());
    }
    (0..count)
        .map(|i| {
            let o = 0x40 + i * stride;
            let (size, offset, name) = (num(d, o + 8, 8).ok_or_else(bad)?, num(d, o + 16, 4).ok_or_else(bad)?, num(d, o + 20, 4).ok_or_else(bad)?);
            match offset.checked_add(size) {
                Some(end) if end <= d.len() => Ok(Entry { name: utf16(d, name), offset, size }),
                _ => Err(bad()),
            }
        })
        .collect()
}

pub fn blob<'a>(d: &'a [u8], e: &Entry) -> &'a [u8] {
    &d[e.offset..e.offset + e.size]
}

pub fn decrypt(d: &[u8], e: &Entry, key: &[u8; 16]) -> Option<Vec<u8>> {
    let (iv, ct) = blob(d, e).split_at_checked(16)?;
    let mut buf = ct[..ct.len() / 16 * 16].to_vec();
    cbc::Decryptor::<Aes128>::new(key.into(), iv.into()).decrypt_padded_mut::<NoPadding>(&mut buf).ok()?;
    let pad = *buf.last()? as usize;
    if !(1..=16).contains(&pad) || buf.len() < pad || buf[buf.len() - pad..].iter().any(|b| *b as usize != pad) {
        return None;
    }
    buf.truncate(buf.len() - pad);
    Some(buf)
}

pub fn encrypt(d: &[u8], e: &Entry, plain: &[u8], key: &[u8; 16]) -> Option<Vec<u8>> {
    let iv: [u8; 16] = d.get(e.offset..e.offset + 16)?.try_into().ok()?;
    let pad = 16 - plain.len() % 16;
    let mut buf = plain.to_vec();
    buf.resize(plain.len() + pad, pad as u8);
    let len = buf.len();
    cbc::Encryptor::<Aes128>::new(key.into(), (&iv).into()).encrypt_padded_mut::<NoPadding>(&mut buf, len).ok()?;
    Some([iv.as_slice(), &buf].concat())
}
