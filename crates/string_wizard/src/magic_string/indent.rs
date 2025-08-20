use std::borrow::Cow;

use crate::{CowStr, MagicString};

struct ExcludeSet<'a> {
  exclude: &'a [(usize, usize)],
}

impl<'a> ExcludeSet<'a> {
  fn new(exclude: &'a [(usize, usize)]) -> Self {
    Self { exclude }
  }

  fn contains(&self, index: usize) -> bool {
    self.exclude.iter().any(|s| s.0 <= index && index < s.1)
  }
}

pub fn guess_indentor(source: &str) -> Option<String> {
  let mut tabbed_count = 0;
  let mut spaced_line = vec![];
  for line in source.lines() {
    if line.starts_with('\t') {
      tabbed_count += 1;
    } else if line.starts_with("  ") {
      spaced_line.push(line);
    }
  }

  if tabbed_count == 0 && spaced_line.is_empty() {
    return None;
  }

  if tabbed_count >= spaced_line.len() {
    return Some("\t".to_string());
  }

  let min_space_count = spaced_line
    .iter()
    .map(|line| line.chars().take_while(|c| *c == ' ').count())
    .min()
    .unwrap_or(0);

  let mut indent_str = String::with_capacity(min_space_count);
  for _ in 0..min_space_count {
    indent_str.push(' ');
  }
  Some(indent_str)
}

#[derive(Debug, Default)]
pub struct IndentOptions<'a, 'b> {
  /// MagicString will guess the `indentor` from lines of the source if passed `None`.
  pub indentor: Option<&'a str>,

  /// The reason I use `[u32; 2]` instead of `(u32, u32)` to represent a range of text is that
  /// I want to emphasize that the `[u32; 2]` is the closed interval, which means both the start
  /// and the end are included in the range.
  pub exclude: &'b [(usize, usize)],
}

impl MagicString<'_> {
  fn guessed_indentor(&mut self) -> &str {
    let guessed_indentor = self
      .guessed_indentor
      .get_or_init(|| guess_indentor(self.source).unwrap_or_else(|| "\t".to_string()));
    guessed_indentor
  }

  pub fn indent(&mut self) -> &mut Self {
    self.indent_with(IndentOptions { indentor: None, ..Default::default() })
  }

  pub fn indent_with(&mut self, opts: IndentOptions) -> &mut Self {
    if opts.indentor.is_some_and(|s| s.is_empty()) {
      return self;
    }
    struct IndentReplacer {
      should_indent_next_char: bool,
      indentor: String,
    }

    fn indent_frag(frag: &mut CowStr, indent_replacer: &mut IndentReplacer) {
      let mut indented = String::new();
      for char in frag.chars() {
        if char == '\n' {
          indent_replacer.should_indent_next_char = true;
        } else if char != '\r' && indent_replacer.should_indent_next_char {
          indent_replacer.should_indent_next_char = false;
          indented.push_str(&indent_replacer.indentor);
        }
        indented.push(char);
      }
      *frag = Cow::Owned(indented);
    }

    let indentor = opts.indentor.unwrap_or_else(|| self.guessed_indentor());

    let mut indent_replacer =
      IndentReplacer { should_indent_next_char: true, indentor: indentor.to_string() };

    for intro_frag in self.intro.iter_mut() {
      indent_frag(intro_frag, &mut indent_replacer)
    }

    let exclude_set = ExcludeSet::new(opts.exclude);

    let mut next_chunk_id = Some(self.first_chunk_idx);
    let mut char_index = 0;
    while let Some(chunk_idx) = next_chunk_id {
      // Make sure the `next_chunk_id` is updated before we split the chunk. Otherwise, we
      // might process the same chunk twice.
      next_chunk_id = self.chunks[chunk_idx].next;
      if let Some(edited_content) = self.chunks[chunk_idx].edited_content.as_mut() {
        if !exclude_set.contains(char_index) {
          indent_frag(edited_content, &mut indent_replacer);
        }
      } else {
        let chunk = &self.chunks[chunk_idx];
        let mut line_starts = vec![];
        char_index = chunk.start();
        let chunk_end = chunk.end();
        for char in chunk.span.text(self.source).chars() {
          debug_assert!(self.source.is_char_boundary(char_index));
          if !exclude_set.contains(char_index) {
            if char == '\n' {
              indent_replacer.should_indent_next_char = true;
            } else if char != '\r' && indent_replacer.should_indent_next_char {
              indent_replacer.should_indent_next_char = false;
              debug_assert!(!line_starts.contains(&char_index));
              line_starts.push(char_index);
            }
          }
          char_index += char.len_utf8();
        }
        for line_start in line_starts {
          self.prepend_right(line_start, indent_replacer.indentor.clone());
        }
        char_index = chunk_end;
      }
    }

    for frag in self.outro.iter_mut() {
      indent_frag(frag, &mut indent_replacer)
    }

    self
  }
}
