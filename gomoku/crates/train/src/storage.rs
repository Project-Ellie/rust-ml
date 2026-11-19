//! Length-delimited bincode storage shared by dataset-like corpora
//!

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// the records 4 by little_endian prefix
pub const LENGTH_PREFIX_BYTES: usize = 4;

/// just in case the prefix got corrupted
pub const MAX_RECORD_LEN: usize = 1 << 20;

/// Write records to one new length-delimited bincode shard.
///
/// The destination is opened with create-new semantics, so an existing shard
/// is never truncated. The file is created before encoding begins; an encoding
/// or I/O failure can therefore leave a partial shard behind.
///
/// # Errors
///
/// Returns an error if the file cannot be created, a record exceeds `u32`
/// length, bincode encoding fails, or writing fails.
pub fn write_shard<T: Serialize>(records: &[T], path: &Path) -> io::Result<()> {
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);

    for record in records {
        let bytes = bincode::serde::encode_to_vec(record, bincode::config::standard())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let len = u32::try_from(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "encoded record exceeds the u32 length limit",
            )
        })?;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&bytes)?;
    }
    writer.flush()
}

/// Read every length-delimited bincode record from one shard, in order.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, a prefix or payload is
/// truncated, a payload exceeds [`MAX_RECORD_LEN`], or bincode decoding fails.
pub fn read_shard<T: DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut len_buf = [0u8; LENGTH_PREFIX_BYTES];
    loop {
        match reader.read(&mut len_buf)? {
            0 => break,
            n => reader.read_exact(&mut len_buf[n..])?,
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > MAX_RECORD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("share record length {len} exceeds maximum allowed {MAX_RECORD_LEN}"),
            ));
        }
        let mut bytes = vec![0u8; len];
        reader.read_exact(&mut bytes)?;

        let (record, _): (T, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        records.push(record);
    }
    Ok(records)
}
