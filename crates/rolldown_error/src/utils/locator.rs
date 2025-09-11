pub struct ByteLocator {
  line_starts: Vec<usize>,
}

impl ByteLocator {
  pub fn new(source: &str) -> Self {
    Self {
      line_starts: std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect(),
    }
  }

  /// line: 0-based column: 0-based
  pub fn byte_offset(&self, line: usize, column: usize) -> usize {
    self.line_starts[line] + column
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
}
