pub struct ByteLocator {
  /// Byte offset of the start of each line.
  line_starts: Vec<usize>,
  /// UTF-16 offset (from the start of the file) of the start of each line,
  /// parallel to `line_starts`.
  line_utf16_starts: Vec<usize>,
}

impl ByteLocator {
  pub fn new(source: &str) -> Self {
    let mut line_starts = vec![0usize];
    let mut line_utf16_starts = vec![0usize];
    let mut utf16 = 0usize;
    for (byte_idx, ch) in source.char_indices() {
      utf16 += ch.len_utf16();
      if ch == '\n' {
        line_starts.push(byte_idx + 1);
        line_utf16_starts.push(utf16);
      }
    }
    Self { line_starts, line_utf16_starts }
  }

  /// line: 0-based column: 0-based
  pub fn byte_offset(&self, line: usize, column: usize) -> usize {
    if line >= self.line_starts.len() {
      // Return last position or handle error appropriately
      return self.line_starts.last().copied().unwrap_or(0) + column;
    }
    self.line_starts[line] + column
  }

  /// Map a byte offset to `(1-based line, 0-based utf-16 column, utf-16 offset
  /// from the start of the file)`.
  ///
  /// The line is found with a binary search over the precomputed line starts, so
  /// locating N offsets in one source is O(N log N) rather than the O(N^2) of
  /// rescanning from offset 0 for each one.
  pub fn locate_utf16(&self, source: &str, byte_offset: usize) -> (usize, usize, usize) {
    // `line_starts` always begins with 0, so `partition_point` is >= 1 and the
    // `- 1` cannot underflow.
    let line_idx = self.line_starts.partition_point(|&start| start <= byte_offset) - 1;
    let line_byte_start = self.line_starts[line_idx];
    let line_utf16_start = self.line_utf16_starts[line_idx];
    // Clamp so a (malformed) out-of-range offset degrades to the end-of-file
    // position instead of yielding column 0.
    let end = byte_offset.min(source.len());
    let column: usize = source
      .get(line_byte_start..end)
      .map(|within_line| within_line.chars().map(char::len_utf16).sum())
      .unwrap_or(0);
    (line_idx + 1, column, line_utf16_start + column)
  }
}

#[cfg(test)]
mod test_locator {
  #[test]
  fn line_column_to_byte_offset() {
    use super::ByteLocator;
    let source = "abc\ndef\ncghi";
    assert_eq!(ByteLocator::new(source).byte_offset(0, 0), 0);
    assert_eq!(ByteLocator::new(source).byte_offset(1, 0), 4);
  }

  #[test]
  fn byte_offset_to_location() {
    use super::ByteLocator;
    // Returns (1-based line, 0-based utf-16 column, utf-16 offset from file start).
    let source = "abc\ndef\ncghi";
    let locator = ByteLocator::new(source);
    assert_eq!(locator.locate_utf16(source, 0), (1, 0, 0));
    assert_eq!(locator.locate_utf16(source, 3), (1, 3, 3)); // the '\n' on line 1
    assert_eq!(locator.locate_utf16(source, 4), (2, 0, 4)); // start of line 2
    assert_eq!(locator.locate_utf16(source, 5), (2, 1, 5));

    // Columns/offsets are utf-16, so a surrogate pair (`💣`) counts as 2 units.
    let unicode = "ß💣\n💣ß";
    let locator = ByteLocator::new(unicode);
    assert_eq!(locator.locate_utf16(unicode, 0), (1, 0, 0));
    assert_eq!(locator.locate_utf16(unicode, 7), (2, 0, 4)); // start of line 2 ('💣')
    assert_eq!(locator.locate_utf16(unicode, 11), (2, 2, 6)); // 'ß' after the bomb
  }

  /// Straightforward, independent reference for [`ByteLocator::locate_utf16`] used
  /// to validate it. `byte_offset` must fall on a char boundary.
  fn reference_locate(source: &str, byte_offset: usize) -> (usize, usize, usize) {
    let end = byte_offset.min(source.len());
    let prefix = &source[..end];
    let utf16_position: usize = prefix.chars().map(char::len_utf16).sum();
    let line0 = prefix.matches('\n').count();
    let line_byte_start = prefix.rfind('\n').map_or(0, |i| i + 1);
    let column: usize = source[line_byte_start..end].chars().map(char::len_utf16).sum();
    (line0 + 1, column, utf16_position)
  }

  fn assert_matches_reference(source: &str) {
    use super::ByteLocator;
    let locator = ByteLocator::new(source);
    for byte_offset in 0..=source.len() {
      if !source.is_char_boundary(byte_offset) {
        continue;
      }
      assert_eq!(
        locator.locate_utf16(source, byte_offset),
        reference_locate(source, byte_offset),
        "mismatch at byte offset {byte_offset} of {source:?}"
      );
    }
    // An out-of-range offset clamps to the end-of-file position.
    let past_end = source.len() + 5;
    assert_eq!(
      locator.locate_utf16(source, past_end),
      reference_locate(source, past_end),
      "mismatch at out-of-range offset {past_end} of {source:?}"
    );
  }

  #[test]
  fn locate_utf16_matches_linear_reference() {
    for source in [
      "",              // empty source / single-entry table + out-of-range clamp
      "a\nbc\n\ndef",  // multiple lines incl. an empty one: binary-search line lookup
      "café\n😀xy\nz", // 2-byte accent + astral emoji across lines: cumulative utf16 table
    ] {
      assert_matches_reference(source);
    }
  }
}
