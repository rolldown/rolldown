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
    // A zero-width range has nothing to move. It also breaks the lookups below — the chunk
    // ending at `start` is the one *before* the range — and used to self-cycle two chunks.
    // Deliberately checked after the containment guard above: `relocate(n, n, n)` keeps the
    // "inside itself" error, matching both magic-string and this method's previous behavior.
    // Every other zero-width move is a no-op (magic-string corrupts on those, so there is no
    // upstream behavior to mirror).
    if start == end {
      return Ok(self);
    }
    if start > end {
      return Err(format!("end must be greater than start, got start: {start}, end: {end}"));
    }
    // Past-the-end positions have no chunk to look up in `chunk_by_end`; indexing the map
    // with one was a panic.
    if end > self.source.len() as u32 {
      return Err(format!(
        "Cannot move the range ({start}, {end}): it is out of bounds (source length is {})",
        self.source.len()
      ));
    }

    self.split_at(start)?;
    self.split_at(end)?;
    self.split_at(to)?;

    let first_idx = self.chunk_by_start[&start];
    let last_idx = self.chunk_by_end[&end];

    let old_left_idx = self.chunks[first_idx].prev;
    let old_right_idx = self.chunks[last_idx].next;

    // The rewiring below lifts `first..=last` out of the list as one segment and splices it
    // back in elsewhere. That is only sound if the chunks covering `[start, end)` still sit
    // next to each other in list order, in original order. An earlier move can break that —
    // after `relocate(0, 2, 3)` on "abc" the list is C -> A -> B, so `relocate(1, 3, 0)`
    // selects first = B (the list tail) and last = C (the head), with A wedged between them.
    //
    // This has to be caught here, before the rewiring: that rewiring is not atomic, and
    // splicing a non-contiguous "segment" leaves a chunk pointing at itself (`to_string`
    // then loops forever) or silently drops content. Walk the range in original order and
    // require each step to also be the list successor.
    let mut idx = first_idx;
    while idx != last_idx {
      // Chunks tile the original source and `end` is a chunk boundary, so stepping by
      // original position from `start` always reaches `last_idx` before running off the end.
      let next_in_original_order = self.chunk_by_start[&self.chunks[idx].end()];
      if self.chunks[idx].next != Some(next_in_original_order) {
        return Err(format!(
          "Cannot move the range ({start}, {end}): an earlier move left it non-contiguous"
        ));
      }
      idx = next_in_original_order;
    }

    let new_right_idx = self.chunk_by_start.get(&to).copied();

    // The range already sits exactly where it would be spliced: the chunk at `to` is the
    // range's own list successor. This also covers `to` at the end of the string with the
    // range already ending the list (both sides `None`). Proceeding would make `new_left`
    // the range's own last chunk and the rewiring would link the range to itself.
    if new_right_idx == old_right_idx {
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
      // A contiguous range open at both ends would be the entire list — i.e. the entire
      // string — which the checks above reject or no-op, so a successor must exist. Returning
      // an error here would corrupt the list, so assert instead of `?`.
      debug_assert!(self.chunks[last_idx].next.is_some(), "open-ended move should be rejected");
      if let Some(next_idx) = self.chunks[last_idx].next {
        self.first_chunk_idx = next_idx;
      }
    }
    if self.chunks[last_idx].next.is_none() {
      // If the `last_idx` is the last chunk, then we need to update the `last_chunk_idx`.
      debug_assert!(self.chunks[first_idx].prev.is_some(), "open-ended move should be rejected");
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
