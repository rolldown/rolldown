use std::io::{StdoutLock, Write as _};

use flate2::{Compression, write::GzEncoder};

pub const COMPRESSIBLE_ASSETS: [&str; 7] =
  [".html", ".json", ".svg", ".txt", ".xml", ".xhtml", ".wasm"];

pub const GROUPS: [AssetGroup; 3] = [AssetGroup::Assets, AssetGroup::Css, AssetGroup::JS];

#[derive(PartialEq, Eq)]
pub enum AssetGroup {
  JS,
  Css,
  Assets,
}

pub struct LogEntry<'a> {
  pub name: &'a str,
  pub size: usize,
  pub group: AssetGroup,
  pub map_size: Option<usize>,
  pub compressed_size: Option<usize>,
}

#[allow(clippy::cast_precision_loss)]
pub fn display_size(size: usize) -> String {
  format!("{:.2} kB", size as f64 / 1000.0)
}

pub fn compute_gzip_size(bytes: &[u8]) -> Option<usize> {
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  if encoder.write_all(bytes).is_err() {
    return None;
  }
  encoder.finish().ok().map(|compressed| compressed.len())
}

#[inline]
pub fn clear_line() -> StdoutLock<'static> {
  let mut lock = std::io::stdout().lock();
  let _ = write!(&mut lock, "\x1b[2K\r");
  let _ = lock.flush();
  lock
}

#[inline]
pub fn write_line(message: &str) {
  let mut lock = clear_line();

  let message = terminal_size::terminal_size()
    .map(|(width, _)| width.0 as usize)
    .map_or(message, |width| if message.len() < width { message } else { &message[..width] });

  let _ = write!(&mut lock, "{message}");
  let _ = lock.flush();
}

#[inline]
pub fn log_info(message: &str) {
  let mut lock = std::io::stdout().lock();
  let _ = write!(&mut lock, "{message}");
  let _ = lock.flush();
}
