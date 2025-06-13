pub fn line_starts(source: &str) -> impl '_ + Iterator<Item = usize> {
  std::iter::once(0).chain(source.match_indices('\n').map(|(i, _)| i + 1))
}

pub struct ByteLocator {
  line_starts: Vec<usize>,
}

impl ByteLocator {
  pub fn new(source: &str) -> Self {
    Self { line_starts: line_starts(source).collect() }
  }

  /// line: 0-based
  /// column: 0-based
  pub fn byte_offset(&self, line: usize, column: usize) -> usize {
    self.line_starts[line] + column
  }
}

/// Creating a ByteLocator is none trivial, if you want to query offset multiple times for same
/// source, please creating a `ByteLocator` and calling `byte_offset`
pub fn line_column_to_byte_offset(source: &str, line: usize, column: usize) -> usize {
  ByteLocator::new(source).byte_offset(line, column)
}

mod test_locator {
  #[test]
  fn line_column_to_byte_offset() {
    use super::ByteLocator;
    let source = "abc\ndef\ncghi";
    assert_eq!(ByteLocator::new(source).byte_offset(0, 0), 0);
    assert_eq!(ByteLocator::new(source).byte_offset(1, 0), 4);
  }
}
