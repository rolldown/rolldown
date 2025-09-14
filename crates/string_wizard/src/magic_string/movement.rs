use crate::MagicString;

use super::update::UpdateOptions;

impl MagicString<'_> {
  pub fn remove(&mut self, start: usize, end: usize) -> &mut Self {
    self.inner_update_with(
      start,
      end,
      "".into(),
      UpdateOptions { keep_original: false, overwrite: true },
      false,
    );

    self
  }

  /// Moves the characters from start and end to index. Returns this.
  // `move` is reserved keyword in Rust, so we use `relocate` instead.
  pub fn relocate(&mut self, start: usize, end: usize, to: usize) -> &mut Self {
    if to >= start && to <= end {
      panic!("Cannot relocate a selection inside itself")
    }

    self.split_at(start);
    self.split_at(end);
    self.split_at(to);

    let first_idx = self.chunk_by_start[&start];
    let last_idx = self.chunk_by_end[&end];

    let old_left_idx = self.chunks[first_idx].prev;
    let old_right_idx = self.chunks[last_idx].next;

    let new_right_idx = self.chunk_by_start.get(&to).copied();

    // `new_right_idx` is `None` means that the `to` index is at the end of the string.
    // Moving chunks which contain the last chunk to the end is meaningless.
    if new_right_idx.is_none() && last_idx == self.last_chunk_idx {
      return self;
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
      self.first_chunk_idx = self.chunks[last_idx].next.unwrap();
    }
    if self.chunks[last_idx].next.is_none() {
      // If the `last_idx` is the last chunk, then we need to update the `last_chunk_idx`.
      self.last_chunk_idx = self.chunks[first_idx].prev.unwrap();
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

    self
  }
}
