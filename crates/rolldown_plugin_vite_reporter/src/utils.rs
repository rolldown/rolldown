use std::{
  io::{StdoutLock, Write},
  sync::OnceLock,
};

use flate2::{Compression, write::GzEncoder};
use num_format::{Locale, ToFormattedString as _};
use owo_colors::OwoColorize as _;

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

/// Returns whether colored output should be emitted on stdout.
///
/// This replicates the precedence used by the `supports-color` crate (which
/// `owo-colors`' `if_supports_color` relies on) for the common cases, without
/// pulling in that crate. The result is computed once and cached for the
/// lifetime of the process, matching `supports_color::on_cached`.
///
/// Precedence (highest first):
/// 1. `FORCE_COLOR` set to anything other than `"0"`/`"false"` -> color on.
/// 2. `NO_COLOR` set to a non-empty, non-`"0"` value, or `TERM=dumb` -> color off.
/// 3. otherwise, color on only when stdout is a terminal.
pub fn should_color() -> bool {
  fn compute() -> bool {
    if let Ok(force) = std::env::var("FORCE_COLOR") {
      return !matches!(force.as_str(), "0" | "false");
    }
    if matches!(std::env::var("NO_COLOR"), Ok(value) if !value.is_empty() && value != "0") {
      return false;
    }
    if matches!(std::env::var("TERM"), Ok(term) if term == "dumb") {
      return false;
    }
    std::io::IsTerminal::is_terminal(&std::io::stdout())
  }

  static CACHE: OnceLock<bool> = OnceLock::new();
  *CACHE.get_or_init(compute)
}

/// Applies the `apply` color transformation to `value` when stdout supports
/// color, otherwise returns the plain value. Mirrors
/// `value.if_supports_color(Stream::Stdout, apply)` but gates on the local
/// [`should_color`] check instead of the `supports-color` crate.
///
/// `apply` renders the colored form to a `String` (e.g. `|t| t.green().to_string()`)
/// so that chained `owo-colors` styles, which borrow intermediate temporaries,
/// don't escape the closure.
#[inline]
pub fn paint<T>(value: T, apply: impl FnOnce(&T) -> String) -> String
where
  T: std::fmt::Display,
{
  if should_color() { apply(&value) } else { value.to_string() }
}

/// Convenience wrappers around [`paint`] for the color methods used by the
/// reporter, keeping the call sites terse.
pub fn dimmed(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.dimmed().to_string())
}

pub fn green(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.green().to_string())
}

pub fn cyan(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.cyan().to_string())
}

pub fn magenta(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.magenta().to_string())
}

pub fn bold_yellow(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.bold().yellow().to_string())
}

pub fn bold_dimmed(value: impl std::fmt::Display) -> String {
  paint(value, |t| t.bold().dimmed().to_string())
}
