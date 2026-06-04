use std::path::Path;

pub(crate) fn file_uri(path: &Path) -> String {
    format!("file://{}", percent_encode_path(path))
}

#[cfg(unix)]
fn percent_encode_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    percent_encode_bytes(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn percent_encode_path(path: &Path) -> String {
    percent_encode_bytes(path.to_string_lossy().as_bytes())
}

fn percent_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
