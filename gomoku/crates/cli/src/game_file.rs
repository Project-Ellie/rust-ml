//! Bounded study-file reads and same-directory replacement writes
//!
use crate::session::record::MAX_RECORD_BYTES;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Read at most the native record limit plus one sentinel byte.
///
/// # Errors
/// Returns filesystem, size, or UTF-8 errors as displayable text.
pub fn read(path: &Path) -> Result<String, String> {

    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take((MAX_RECORD_BYTES + 1) as u64).read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("Study exceeds 4MiB".into());
    }

    String::from_utf8(bytes).map_err(|e| format!("Study is not UTF-8: {e}" ))
}

fn create_temp(path: &Path) -> Result<(PathBuf, File), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or("destination needs a filename")?;
    for _ in 0..128 {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(".tmp-{}-{serial}", std::process::id()));
        let temp = parent.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", temp.display())),
        }
    }
    Err("could not create a unique study temporary file".into())
}

/// Replace a study file only after writing, flushing, syncing and closing.
///
/// # Errors
/// Returns size or filesystem errors; removes an unsuccessful temporary file.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if text.len() > MAX_RECORD_BYTES {
        return Err("study exceeds 4 MiB".into());
    }
    let (temp, mut file) = create_temp(path)?;
    let written = file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}