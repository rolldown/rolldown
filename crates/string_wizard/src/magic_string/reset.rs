use crate::MagicString;

impl<'text> MagicString<'text> {
  /// Resets the portion of the string from `start` to `end` to its original content.
  /// This undoes any modifications (updates, overwrites, intro/outro additions) made to that range.
  ///
  /// # Errors
  /// - If `start` is greater than to `end`
  /// - If the range is out of bounds
  pub fn reset(&mut self, start: u32, end: u32) -> Result<&mut Self, String> {
    if start == end {
      return Ok(self);
    }

    if start > end {
      return Err("end must be greater than start".to_string());
    }

    if (end as usize) > self.source.len() {
      return Err("Character is out of bounds".to_string());
    }

    self.split_at(start)?;
    self.split_at(end)?;

    let mut chunk_idx = self.chunk_by_start.get(&start).copied();

    while let Some(idx) = chunk_idx {
      let chunk = &mut self.chunks[idx];
      chunk.reset();
      let chunk_end = chunk.end();
      chunk_idx = if end > chunk_end { self.chunk_by_start.get(&chunk_end).copied() } else { None };
    }

    Ok(self)
  }
}
