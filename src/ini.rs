use std::{fs, io, path::Path};

fn value_offset(line: &str, key: &str) -> Option<usize> {
    let rest = line.strip_prefix(key)?.trim_start_matches([' ', '\t']).strip_prefix('=')?;
    Some(line.len() - rest.trim_start_matches([' ', '\t']).len())
}

pub fn get(path: &Path, key: &str) -> Option<String> {
    let text = String::from_utf8_lossy(&fs::read(path).ok()?).into_owned();
    text.lines().find_map(|l| value_offset(l, key).map(|o| l[o..].trim().to_string()))
}

pub fn set(path: &Path, key: &str, value: &str) -> io::Result<()> {
    let data = fs::read(path)?;
    let mut out = Vec::with_capacity(data.len() + value.len());
    let mut done = false;
    for line in data.split_inclusive(|b| *b == b'\n') {
        let text = std::str::from_utf8(line).unwrap_or("");
        let body = text.trim_end_matches(['\r', '\n']);
        match value_offset(body, key).filter(|_| !done) {
            Some(o) => {
                out.extend_from_slice(&line[..o]);
                out.extend_from_slice(value.as_bytes());
                out.extend_from_slice(text[body.len()..].as_bytes());
                done = true;
            }
            None => out.extend_from_slice(line),
        }
    }
    fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_one_value_and_keeps_line_endings() {
        let p = std::env::temp_dir().join("souls-like-ini-test.ini");
        fs::write(&p, b"; c\r\nsave_file_extension = va2\r\nsave_file_extension2 = x\r\nother = 1\r\n").unwrap();
        assert_eq!(get(&p, "save_file_extension").as_deref(), Some("va2"));
        set(&p, "save_file_extension", "co2").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"; c\r\nsave_file_extension = co2\r\nsave_file_extension2 = x\r\nother = 1\r\n");
    }
}
