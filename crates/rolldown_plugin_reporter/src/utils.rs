use std::io::{StdoutLock, Write as _};

#[inline]
#[allow(clippy::print_stdout)]
pub fn clear_current_line() -> StdoutLock<'static> {
  let mut lock = std::io::stdout().lock();
  write!(&mut lock, "\x1B[2K\r").unwrap(); // clear current line and move cursor to the beginning
  lock.flush().unwrap();
  lock
}

#[inline]
#[allow(clippy::print_stdout)]
pub fn write_line(line: &str) {
  let mut lock = clear_current_line();
  write!(&mut lock, "{line}",).unwrap();
  lock.flush().unwrap();
}
