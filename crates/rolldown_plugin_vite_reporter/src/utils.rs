use std::io::{StdoutLock, Write};

use flate2::{Compression, write::GzEncoder};
use num_format::{Locale, ToFormattedString as _};

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
  format!("{}.{:02} kB", quotient.to_formatted_string(&Locale::en), remainder)
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

/// Joins a vector of items with a separator, showing only the first `limit` items
/// and adding "..." if there are more.
pub fn join_with_limit<T: AsRef<str>>(items: &[T], separator: &str, limit: usize) -> String {
  debug_assert!(limit > 0, "limit must be greater than 0");
  if items.len() <= limit {
    items.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(separator)
  } else {
    let mut result = items[..limit].iter().map(AsRef::as_ref).collect::<Vec<_>>().join(separator);
    result.push_str(separator);
    result.push_str("...");
    result
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_join_with_limit_less_than_limit() {
    let items = vec!["a", "b", "c"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c");
  }

  #[test]
  fn test_join_with_limit_equal_to_limit() {
    let items = vec!["a", "b", "c", "d", "e"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c, d, e");
  }

  #[test]
  fn test_join_with_limit_more_than_limit() {
    let items = vec!["a", "b", "c", "d", "e", "f", "g"];
    assert_eq!(join_with_limit(&items, ", ", 5), "a, b, c, d, e, ...");
  }

  #[test]
  fn test_join_with_limit_custom_separator() {
    let items = vec!["a", "b", "c", "d", "e", "f"];
    assert_eq!(join_with_limit(&items, " | ", 3), "a | b | c | ...");
  }

  #[test]
  fn test_join_with_limit_empty_vector() {
    let items: Vec<&str> = vec![];
    assert_eq!(join_with_limit(&items, ", ", 5), "");
  }
}
