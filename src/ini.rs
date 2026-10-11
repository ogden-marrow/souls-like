//! Minimal `key = value` ini editing that leaves everything else (comments, CRLF endings) alone.

use std::{fs, io, path::Path};

fn match_value(line: &str, key: &str) -> Option<usize> {
    let rest = line.strip_prefix(key)?;
    let trimmed = rest.trim_start_matches([' ', '\t']);
    if trimmed.starts_with('=') {
        // byte offset of the value start (after '=' and following blanks)
        let after_eq = &trimmed[1..];
        let value = after_eq.trim_start_matches([' ', '\t']);
        Some(line.len() - value.len())
    } else {
        None
    }
}

pub fn get(path: &Path, key: &str) -> Option<String> {
    let data = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&data);
    for line in text.lines() {
        if let Some(off) = match_value(line, key) {
            return Some(line[off..].trim().to_string());
        }
    }
    None
}

pub fn set(path: &Path, key: &str, value: &str) -> io::Result<()> {
    let data = fs::read(path)?;
    let mut out = Vec::with_capacity(data.len() + value.len());
    let mut done = false;
    for line in data.split_inclusive(|b| *b == b'\n') {
        if !done {
            if let Ok(s) = std::str::from_utf8(line) {
                let body = s.trim_end_matches(['\r', '\n']);
                if let Some(off) = match_value(body, key) {
                    out.extend_from_slice(&line[..off]);
                    out.extend_from_slice(value.as_bytes());
                    out.extend_from_slice(s[body.len()..].as_bytes());
                    done = true;
                    continue;
                }
            }
        }
        out.extend_from_slice(line);
    }
    fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_crlf_and_other_lines() {
        let dir = std::env::temp_dir().join("souls-like-ini-test");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.ini");
        fs::write(&p, b"; c\r\nsave_file_extension = va2\r\nother = 1\r\n").unwrap();
        assert_eq!(get(&p, "save_file_extension").as_deref(), Some("va2"));
        set(&p, "save_file_extension", "co2").unwrap();
        assert_eq!(
            fs::read(&p).unwrap(),
            b"; c\r\nsave_file_extension = co2\r\nother = 1\r\n"
        );
        set(&p, "other", "2").unwrap();
        assert_eq!(get(&p, "other").as_deref(), Some("2"));
    }
}
