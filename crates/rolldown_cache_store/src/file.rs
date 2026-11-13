use std::{
  ffi::OsString,
  fs::{self, File},
  io::{self, BufWriter, Write},
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustc_hash::FxHashSet;
use xxhash_rust::xxh3::xxh3_64;

use crate::format::{self, Footer, HEADER_LEN, TRAILER_LEN};

const SEGMENT_PREFIX: &str = "seg-";
const SEGMENT_SUFFIX: &str = ".bin";
const TEMP_PREFIX: &str = ".seg-";
const TEMP_SUFFIX: &str = ".tmp";

pub fn system_now_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Names of segment files in `dir`. A missing dir has none.
pub fn list_segment_names(dir: &Path) -> io::Result<FxHashSet<OsString>> {
  let entries = match fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(FxHashSet::default()),
    Err(err) => return Err(err),
  };
  Ok(
    entries
      .filter_map(Result::ok)
      .map(|entry| entry.file_name())
      .filter(|name| {
        name.to_str().is_some_and(|s| s.starts_with(SEGMENT_PREFIX) && s.ends_with(SEGMENT_SUFFIX))
      })
      .collect(),
  )
}

/// Removes files in `dir` whose names match `matches` and that were last modified before
/// `cutoff_ms`. Errors are ignored: another process may remove the same files.
pub fn remove_old_files(dir: &Path, cutoff_ms: u64, matches: impl Fn(&str) -> bool) {
  let Ok(entries) = fs::read_dir(dir) else { return };
  for entry in entries.filter_map(Result::ok) {
    let name = entry.file_name();
    if !name.to_str().is_some_and(&matches) {
      continue;
    }
    let modified_ms = entry
      .metadata()
      .and_then(|m| m.modified())
      .ok()
      .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
      .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    if modified_ms.is_some_and(|ms| ms < cutoff_ms) {
      let _ = fs::remove_file(entry.path());
    }
  }
}

pub fn is_temp_name(name: &str) -> bool {
  name.starts_with(TEMP_PREFIX) && name.ends_with(TEMP_SUFFIX)
}

/// Reads the footer and the file length of a segment. `None` for a damaged, incomplete or
/// foreign file.
pub fn read_footer(path: &Path) -> Option<(Footer, u64)> {
  let file = File::open(path).ok()?;
  let file_len = file.metadata().ok()?.len();
  let trailer_start = file_len.checked_sub(TRAILER_LEN as u64)?;
  let mut trailer = [0; TRAILER_LEN];
  read_exact_at(&file, &mut trailer, trailer_start).ok()?;
  let (footer_len, checksum) = format::parse_trailer(&trailer)?;
  let footer_start = trailer_start.checked_sub(footer_len)?;
  if footer_start < HEADER_LEN as u64 {
    return None;
  }
  let mut footer = vec![0; usize::try_from(footer_len).ok()?];
  read_exact_at(&file, &mut footer, footer_start).ok()?;
  Some((format::decode_footer(&footer, checksum, footer_start)?, file_len))
}

/// `Ok(None)` when the bytes do not match the checksum.
pub fn read_value(
  file: &File,
  offset: u64,
  len: u64,
  checksum: u64,
) -> io::Result<Option<Vec<u8>>> {
  let len = usize::try_from(len).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
  let mut buf = vec![0; len];
  read_exact_at(file, &mut buf, offset)?;
  Ok((xxh3_64(&buf) == checksum).then_some(buf))
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
  std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
  use std::os::windows::fs::FileExt;
  while !buf.is_empty() {
    match file.seek_read(buf, offset) {
      Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
      Ok(n) => {
        buf = &mut buf[n..];
        offset += n as u64;
      }
      Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
      Err(err) => return Err(err),
    }
  }
  Ok(())
}

#[cfg(not(any(unix, windows)))]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
  use std::io::{Read, Seek, SeekFrom};
  let mut file = file;
  file.seek(SeekFrom::Start(offset))?;
  file.read_exact(buf)
}

fn process_id() -> u32 {
  // `std::process::id` panics on targets without processes, such as WASI.
  #[cfg(any(unix, windows))]
  {
    std::process::id()
  }
  #[cfg(not(any(unix, windows)))]
  {
    0
  }
}

/// A name no other flush uses, in this process or in another one.
fn unique_stem(created_ms: u64) -> String {
  static COUNTER: AtomicU64 = AtomicU64::new(0);
  let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
  let pid = process_id();
  let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_nanos();
  let mut seed = Vec::with_capacity(48);
  seed.extend_from_slice(&nanos.to_le_bytes());
  seed.extend_from_slice(&pid.to_le_bytes());
  seed.extend_from_slice(&counter.to_le_bytes());
  seed.extend_from_slice(&(std::ptr::from_ref(&seed) as usize as u64).to_le_bytes());
  let random = xxh3_64(&seed);
  format!("{created_ms:013}-{pid:010}-{counter:06}-{random:016x}")
}

/// Writes one segment to a temp file, then renames it in place. Dropping it before
/// `finish` removes the temp file.
pub struct SegmentWriter {
  dir: PathBuf,
  stem: String,
  temp_path: PathBuf,
  out: Option<BufWriter<File>>,
  pos: u64,
}

impl SegmentWriter {
  pub fn create(dir: &Path, created_ms: u64) -> io::Result<Self> {
    fs::create_dir_all(dir)?;
    let stem = unique_stem(created_ms);
    let temp_path = dir.join(format!("{TEMP_PREFIX}{stem}{TEMP_SUFFIX}"));
    let file = File::options().write(true).create_new(true).open(&temp_path)?;
    let mut writer = Self {
      dir: dir.to_path_buf(),
      stem,
      temp_path,
      out: Some(BufWriter::with_capacity(256 * 1024, file)),
      pos: 0,
    };
    writer.write(&format::header())?;
    Ok(writer)
  }

  /// Returns the offset and the checksum of the value.
  pub fn write_value(&mut self, value: &[u8]) -> io::Result<(u64, u64)> {
    let offset = self.pos;
    self.write(value)?;
    Ok((offset, xxh3_64(value)))
  }

  /// Returns the file name, the path and the file length of the segment.
  pub fn finish(mut self, footer: &Footer) -> io::Result<(OsString, PathBuf, u64)> {
    let footer = format::encode_footer(footer);
    self.write(&footer)?;
    self.write(&format::trailer(&footer))?;
    let out = self.out.take().ok_or_else(|| io::Error::from(io::ErrorKind::Other))?;
    let file = out.into_inner().map_err(io::IntoInnerError::into_error)?;
    file.sync_all()?;
    drop(file);
    let name = format!("{SEGMENT_PREFIX}{}{SEGMENT_SUFFIX}", self.stem);
    let path = self.dir.join(&name);
    fs::rename(&self.temp_path, &path)?;
    Ok((name.into(), path, self.pos))
  }

  fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
    let out = self.out.as_mut().ok_or_else(|| io::Error::from(io::ErrorKind::Other))?;
    out.write_all(bytes)?;
    self.pos += bytes.len() as u64;
    Ok(())
  }
}

impl Drop for SegmentWriter {
  fn drop(&mut self) {
    // After a successful `finish` the temp file is already renamed, so this fails silently.
    self.out = None;
    let _ = fs::remove_file(&self.temp_path);
  }
}
