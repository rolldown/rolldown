use crate::chunk::Chunk;

use super::locator::Locator;

#[derive(Debug, Default)]
pub enum Hires {
  #[default]
  False,
  True,
  Boundary,
}

pub struct SourcemapBuilder {
  hires: Hires,
  generated_code_line: usize,
  /// `generated_code_column` is calculated based on utf-16.
  generated_code_column: usize,
  source_id: u32,
  source_map_builder: oxc_sourcemap::SourceMapBuilder,
}

impl SourcemapBuilder {
  pub fn new(hires: Hires) -> Self {
    Self {
      hires,
      generated_code_line: 0,
      generated_code_column: 0,
      source_id: 0,
      source_map_builder: oxc_sourcemap::SourceMapBuilder::default(),
    }
  }

  pub fn into_source_map(self) -> oxc_sourcemap::SourceMap {
    self.source_map_builder.into_sourcemap()
  }

  pub fn set_source_and_content(&mut self, id: &str, content: &str) {
    self.source_id = self.source_map_builder.set_source_and_content(id, content);
  }

  pub fn add_chunk(
    &mut self,
    chunk: &Chunk,
    chunk_start_utf16: usize,
    locator: &Locator,
    source: &str,
    name: Option<&str>,
  ) {
    let name_id = if chunk.keep_in_mappings {
      name.map(|name| self.source_map_builder.add_name(name))
    } else {
      None
    };
    let mut loc = locator.locate(chunk_start_utf16);
    if let Some(edited_content) = &chunk.edited_content {
      if !edited_content.is_empty() {
        self.source_map_builder.add_token(
          self.generated_code_line as u32,
          self.generated_code_column as u32,
          loc.line as u32,
          loc.column as u32,
          Some(self.source_id),
          name_id,
        );
      }
      self.advance(edited_content);
    } else {
      let chunk_content = chunk.span.text(source);
      let mut new_line = true;
      let mut char_in_hires_boundary = false;
      for char in chunk_content.chars() {
        match char {
          '\n' => {
            loc.bump_line();
            self.bump_line();
            new_line = true;
          }
          _ => {
            if new_line || !matches!(self.hires, Hires::False) {
              if matches!(self.hires, Hires::Boundary) {
                if char.is_alphanumeric() || char == '_' {
                  if !char_in_hires_boundary {
                    self.source_map_builder.add_token(
                      self.generated_code_line as u32,
                      self.generated_code_column as u32,
                      loc.line as u32,
                      loc.column as u32,
                      Some(self.source_id),
                      name_id,
                    );
                    char_in_hires_boundary = true;
                  }
                } else {
                  self.source_map_builder.add_token(
                    self.generated_code_line as u32,
                    self.generated_code_column as u32,
                    loc.line as u32,
                    loc.column as u32,
                    Some(self.source_id),
                    name_id,
                  );
                  char_in_hires_boundary = false;
                }
              } else {
                self.source_map_builder.add_token(
                  self.generated_code_line as u32,
                  self.generated_code_column as u32,
                  loc.line as u32,
                  loc.column as u32,
                  Some(self.source_id),
                  name_id,
                );
              }
            }
            let char_utf16_len = char.len_utf16();
            loc.column += char_utf16_len;
            self.generated_code_column += char_utf16_len;
            new_line = false;
          }
        }
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
    self.generated_code_column += last_line.chars().map(|c| c.len_utf16()).sum::<usize>();
  }

  fn bump_line(&mut self) {
    self.generated_code_line += 1;
    self.generated_code_column = 0;
  }
}
