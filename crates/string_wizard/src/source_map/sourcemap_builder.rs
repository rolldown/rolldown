use rustc_hash::FxHashSet;

use crate::chunk::Chunk;

use super::locator::Locator;

#[derive(Debug, Default)]
pub enum Hires {
  #[default]
  False,
  True,
  Boundary,
}

pub struct SourcemapBuilder<'a> {
  hires: Hires,
  generated_code_line: u32,
  /// `generated_code_column` is calculated based on utf-16.
  generated_code_column: u32,
  source_id: u32,
  source_map_builder: oxc_sourcemap::SourceMapBuilder<'a>,
}

impl<'a> SourcemapBuilder<'a> {
  pub fn new(hires: Hires) -> Self {
    Self {
      hires,
      generated_code_line: 0,
      generated_code_column: 0,
      source_id: 0,
      source_map_builder: oxc_sourcemap::SourceMapBuilder::default(),
    }
  }

  pub fn into_source_map(self) -> oxc_sourcemap::SourceMap<'static> {
    // The oxc builder borrows its strings for `'a`; copy them once into a
    // `'static` sourcemap so the result can be stored independently.
    self.source_map_builder.into_owned_sourcemap().into_inner()
  }

  pub fn set_source_and_content(&mut self, id: &'a str, content: &'a str) {
    self.source_id = self.source_map_builder.set_source_and_content(id, content);
  }

  /// Registers a sourcemap name up front and returns its index in `names`.
  pub fn add_name(&mut self, name: &'a str) -> u32 {
    self.source_map_builder.add_name(name)
  }

  pub fn add_chunk(
    &mut self,
    chunk: &Chunk,
    chunk_start_utf16: u32,
    locator: &Locator,
    source: &str,
    name_id: Option<u32>,
    sourcemap_locations: &FxHashSet<u32>,
  ) {
    let mut loc = locator.locate(chunk_start_utf16);
    if let Some(edited_content) = &chunk.edited_content {
      if edited_content.is_empty() {
        // An empty edit (e.g. `remove`) maps to nothing. (Upstream's `else if
        // (this.pending)` branch here is dead code — `pending` is never assigned.)
      } else {
        // magic-string's `addEdit`: an edit spanning multiple generated lines gets one
        // segment per content line — each pointing at the edit's original position, with
        // the name repeated — except a trailing empty line after a final newline, which
        // gets none. The final line's characters are then advanced over to keep the
        // generated column in sync.
        let bytes = edited_content.as_bytes();
        let mut line_start = 0usize;
        loop {
          self.source_map_builder.add_token(
            self.generated_code_line,
            self.generated_code_column,
            loc.line,
            loc.column,
            Some(self.source_id),
            name_id,
          );
          match memchr::memchr(b'\n', &bytes[line_start..]) {
            // A newline that is not the content's last byte starts another content line,
            // which needs its own segment.
            Some(pos) if line_start + pos < bytes.len() - 1 => {
              self.bump_line();
              line_start += pos + 1;
            }
            _ => break,
          }
        }
        self.advance(&edited_content[line_start..]);
      }
    } else {
      let chunk_content = chunk.span.text(source);
      // Byte offset of the current char in the original source, matched against
      // `sourcemap_locations` (magic-string's `sourcemapLocations.has(originalCharIndex)`,
      // with byte offsets standing in for its UTF-16 indices).
      let mut byte_pos = chunk.start();
      let mut new_line = true;
      let mut char_in_hires_boundary = false;
      for char in chunk_content.chars() {
        match char {
          '\n' => {
            loc.bump_line();
            self.bump_line();
            new_line = true;
            // A newline ends the current word run.
            char_in_hires_boundary = false;
          }
          _ => {
            if new_line
              || !matches!(self.hires, Hires::False)
              || sourcemap_locations.contains(&byte_pos)
            {
              if matches!(self.hires, Hires::Boundary) {
                if char.is_alphanumeric() || char == '_' {
                  if !char_in_hires_boundary {
                    self.source_map_builder.add_token(
                      self.generated_code_line,
                      self.generated_code_column,
                      loc.line,
                      loc.column,
                      Some(self.source_id),
                      name_id,
                    );
                    char_in_hires_boundary = true;
                  }
                } else {
                  self.source_map_builder.add_token(
                    self.generated_code_line,
                    self.generated_code_column,
                    loc.line,
                    loc.column,
                    Some(self.source_id),
                    name_id,
                  );
                  char_in_hires_boundary = false;
                }
              } else {
                self.source_map_builder.add_token(
                  self.generated_code_line,
                  self.generated_code_column,
                  loc.line,
                  loc.column,
                  Some(self.source_id),
                  name_id,
                );
              }
            }
            let char_utf16_len = char.len_utf16() as u32;
            loc.column += char_utf16_len;
            self.generated_code_column += char_utf16_len;
            new_line = false;
          }
        }
        byte_pos += char.len_utf8() as u32;
      }
    }
  }

  pub fn advance(&mut self, content: &str) {
    if content.is_empty() {
      return;
    }
    let mut lines = content.split('\n');

    // SAFETY: In any cases, lines would have at least one element.
    // "".split('\n') would create `[""]`.
    // "\n".split('\n') would create `["", ""]`.
    let last_line = unsafe { lines.next_back().unwrap_unchecked() };
    for _ in lines {
      self.bump_line();
    }
    // Fast path: ASCII strings have 1:1 byte-to-UTF-16 mapping
    self.generated_code_column += if last_line.is_ascii() {
      last_line.len() as u32
    } else {
      last_line.chars().map(|c| c.len_utf16() as u32).sum::<u32>()
    };
  }

  fn bump_line(&mut self) {
    self.generated_code_line += 1;
    self.generated_code_column = 0;
  }
}
