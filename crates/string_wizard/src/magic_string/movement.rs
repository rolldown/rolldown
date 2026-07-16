use crate::{MagicString, chunk::EditOptions};

impl MagicString<'_> {
  /// Removes characters in the range `[start, end)` from the generated output.
  ///
  /// Unlike `update`/`overwrite`, this iterates by original position (via `chunk_by_start`)
  /// rather than by linked-list order, so it works correctly across moved content.
  pub fn remove(&mut self, start: u32, end: u32) -> Result<&mut Self, String> {
    if start == end {
      return Ok(self);
    }
    if start > end {
      return Err(format!("end must be greater than start, got start: {start}, end: {end}"));
    }

    self.split_at(start)?;
    self.split_at(end)?;

    let mut chunk_idx = self.chunk_by_start.get(&start).copied();

    while let Some(idx) = chunk_idx {
      let chunk = &mut self.chunks[idx];
      let chunk_end = chunk.end();
      chunk.edit("".into(), EditOptions { overwrite: true, store_name: false });

      chunk_idx = if end > chunk_end { self.chunk_by_start.get(&chunk_end).copied() } else { None };
    }

    Ok(self)
  }

  /// Moves the characters from start and end to index. Returns this.
  // `move` is reserved keyword in Rust, so we use `relocate` instead.
  pub fn relocate(&mut self, start: u32, end: u32, to: u32) -> Result<&mut Self, String> {
    if to >= start && to <= end {
      return Err("Cannot move a selection inside itself".to_string());
    }

    self.split_at(start)?;
    self.split_at(end)?;
    self.split_at(to)?;

    let first_idx = self.chunk_by_start[&start];
    let last_idx = self.chunk_by_end[&end];

    let old_left_idx = self.chunks[first_idx].prev;
    let old_right_idx = self.chunks[last_idx].next;

    // The moved range occupies both ends of the linked list, so lifting it out would leave no
    // chunk to re-attach to. Reachable once an earlier move has reordered chunks — after
    // `relocate(0, 1, 2)` on "abc" the list is B->A->C, so `relocate(1, 3, 0)` asks to move
    // B..C, which is no longer a contiguous run.
    //
    // This has to be caught here, before the rewiring below: that rewiring is not atomic, and
    // bailing out part-way through leaves a chunk pointing at itself, which makes `to_string`
    // loop forever.
    if old_left_idx.is_none() && old_right_idx.is_none() {
      return Err("Cannot move a range that spans the entire string".to_string());
    }

    let new_right_idx = self.chunk_by_start.get(&to).copied();

    // `new_right_idx` is `None` means that the `to` index is at the end of the string.
    // Moving chunks which contain the last chunk to the end is meaningless.
    if new_right_idx.is_none() && last_idx == self.last_chunk_idx {
      return Ok(self);
    }

    let new_left_idx = new_right_idx
      .map(|idx| self.chunks[idx].prev)
      // If the `to` index is at the end of the string, then the `new_right_idx` will be `None`.
      // In this case, we want to use the last chunk as the left chunk to connect the relocated chunk.
      .unwrap_or(Some(self.last_chunk_idx));

    // Adjust next/prev pointers, this remove the [start, end] range from the old position
    if let Some(old_left_idx) = old_left_idx {
      self.chunks[old_left_idx].next = old_right_idx;
    }
    if let Some(old_right_idx) = old_right_idx {
      self.chunks[old_right_idx].prev = old_left_idx;
    }

    if let Some(new_left_idx) = new_left_idx {
      self.chunks[new_left_idx].next = Some(first_idx);
    }
    if let Some(new_right_idx) = new_right_idx {
      self.chunks[new_right_idx].prev = Some(last_idx);
    }

    if self.chunks[first_idx].prev.is_none() {
      // If the `first_idx` is the first chunk, then we need to update the `first_chunk_idx`.
      // The whole-range check above guarantees a successor exists, so this cannot be `None`
      // — but returning an error here would corrupt the list, so assert instead of `?`.
      debug_assert!(self.chunks[last_idx].next.is_some(), "whole-range move should be rejected");
      if let Some(next_idx) = self.chunks[last_idx].next {
        self.first_chunk_idx = next_idx;
      }
    }
    if self.chunks[last_idx].next.is_none() {
      // If the `last_idx` is the last chunk, then we need to update the `last_chunk_idx`.
      debug_assert!(self.chunks[first_idx].prev.is_some(), "whole-range move should be rejected");
      if let Some(prev_idx) = self.chunks[first_idx].prev {
        self.last_chunk_idx = prev_idx;
      }
      self.chunks[last_idx].next = None;
    }

    if new_left_idx.is_none() {
      self.first_chunk_idx = first_idx;
    }
    if new_right_idx.is_none() {
      self.last_chunk_idx = last_idx;
    }

    self.chunks[first_idx].prev = new_left_idx;
    self.chunks[last_idx].next = new_right_idx;

    Ok(self)
  }

  /// Returns a clone with content outside the specified range removed.
  /// This is equivalent to `clone().remove(0, start).remove(end, original.len())`.
  pub fn snip(&self, start: u32, end: u32) -> Result<Self, String> {
    let mut clone = self.clone();
    if start > 0 {
      clone.remove(0, start)?;
    }
    let original_len = self.source.len() as u32;
    if end < original_len {
      clone.remove(end, original_len)?;
    }
    Ok(clone)
  }
}
