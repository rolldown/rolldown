#[derive(Debug)]
pub struct Locator {
  /// offsets are calculated based on utf-16
  line_offsets: Box<[usize]>,
}

impl Locator {
  pub fn new(source: &str) -> Self {
    let mut line_offsets = vec![];
    let mut line_start_pos = 0;
    for line in source.split('\n') {
      line_offsets.push(line_start_pos);
      line_start_pos += 1 + line.chars().map(|c| c.len_utf16()).sum::<usize>();
    }
    Self { line_offsets: line_offsets.into_boxed_slice() }
  }

  /// Pass the index based on utf-16 and return the [Location] based on utf-16
  pub fn locate(&self, index: usize) -> Location {
    let mut left_cursor = 0;
    let mut right_cursor = self.line_offsets.len();
    while left_cursor < right_cursor {
      let mid = (left_cursor + right_cursor) >> 1;
      if index < self.line_offsets[mid] {
        right_cursor = mid;
      } else {
        left_cursor = mid + 1;
      }
    }
    let line = left_cursor - 1;
    let column = index - self.line_offsets[line];
    Location { line, column }
  }
}

#[derive(Debug, PartialEq)]
pub struct Location {
  pub line: usize,
  // columns are calculated based on utf-16
  pub column: usize,
}

impl Location {
  pub fn bump_line(&mut self) {
    self.line += 1;
    self.column = 0;
  }
}

#[test]
fn basic() {
  let source = "string\nwizard";
  let locator = Locator::new(source);

  assert_eq!(locator.line_offsets[0], 0);
  assert_eq!(locator.line_offsets[1], 7);

  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
  assert_eq!(locator.locate(12), Location { line: 1, column: 5 });
  assert_eq!(locator.locate(7), Location { line: 1, column: 0 });
  assert_eq!(locator.locate(1), Location { line: 0, column: 1 });
  assert_eq!(locator.locate(8), Location { line: 1, column: 1 });
}

#[test]
fn special_chars() {
  let source = "ß💣\n💣ß";
  let locator = Locator::new(source);
  assert_eq!(locator.line_offsets[0], 0);
  assert_eq!(locator.line_offsets[1], 4);

  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
  assert_eq!(locator.locate(4), Location { line: 1, column: 0 });
  assert_eq!(locator.locate(6), Location { line: 1, column: 2 });
}

#[test]
fn edge_cases() {
  let locator = Locator::new("");
  assert_eq!(locator.line_offsets.len(), 1);
  assert_eq!(locator.locate(0), Location { line: 0, column: 0 });
}
