use std::io::{StdoutLock, Write};

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

pub fn display_size(size: usize) -> String {
  let (quotient, remainder) = (size / 1000, (size % 1000) / 10);
  format!("{}.{:02} kB", group_thousands(quotient), remainder)
}

/// Inserts a `,` thousands separator every three digits from the right,
/// matching `num_format`'s `Locale::en` formatting (standard 3-digit grouping).
fn group_thousands(n: usize) -> String {
  let digits = itoa::Buffer::new().format(n).to_string();
  let len = digits.len();
  // 1 separator for every 3 digits beyond the first group.
  let mut out = String::with_capacity(len + (len.saturating_sub(1)) / 3);
  for (i, ch) in digits.bytes().enumerate() {
    if i != 0 && (len - i).is_multiple_of(3) {
      out.push(',');
    }
    out.push(ch as char);
  }
  out
}

struct CountingWriter {
  pub count: usize,
}

impl Write for CountingWriter {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self.count += buf.len();
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

pub fn compute_gzip_size(bytes: &[u8]) -> Option<usize> {
  let mut counter = CountingWriter { count: 0 };
  let mut encoder = GzEncoder::new(&mut counter, Compression::default());
  encoder.write_all(bytes).ok()?;
  encoder.finish().ok()?;
  Some(counter.count)
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
  let _ = writeln!(&mut lock, "{message}");
  let _ = lock.flush();
}

#[cfg(test)]
mod tests {
  use super::{display_size, group_thousands};

  #[test]
  fn test_group_thousands() {
    assert_eq!(group_thousands(0), "0");
    assert_eq!(group_thousands(5), "5");
    assert_eq!(group_thousands(999), "999");
    assert_eq!(group_thousands(1000), "1,000");
    assert_eq!(group_thousands(12345), "12,345");
    assert_eq!(group_thousands(1_234_567), "1,234,567");
    assert_eq!(group_thousands(1_000_000), "1,000,000");
  }

  #[test]
  fn test_display_size() {
    assert_eq!(display_size(0), "0.00 kB");
    assert_eq!(display_size(1_234_560), "1,234.56 kB");
  }
}
