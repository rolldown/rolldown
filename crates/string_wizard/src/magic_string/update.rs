use crate::{CowStr, MagicString, chunk::EditOptions};

#[derive(Debug, Default, Clone)]
pub struct UpdateOptions {
  /// `true` will store the original content in the `name` field of the generated sourcemap.
  pub keep_original: bool,

  /// `true` will clear the `intro` and `outro` for the corresponding range.
  pub overwrite: bool,
}

impl<'text> MagicString<'text> {
  /// A shorthand for `update_with(start, end, content, Default::default())`;
  pub fn update(
    &mut self,
    start: u32,
    end: u32,
    content: impl Into<CowStr<'text>>,
  ) -> Result<&mut Self, String> {
    self.update_with(start, end, content, Default::default())
  }

  pub fn update_with(
    &mut self,
    start: u32,
    end: u32,
    content: impl Into<CowStr<'text>>,
    opts: UpdateOptions,
  ) -> Result<&mut Self, String> {
    self.inner_update_with(start, end, content.into(), opts, true)
  }

  // --- private

  pub(super) fn inner_update_with(
    &mut self,
    start: u32,
    end: u32,
    content: CowStr<'text>,
    opts: UpdateOptions,
    error_if_start_equal_end: bool,
  ) -> Result<&mut Self, String> {
    if error_if_start_equal_end && start == end {
      return Err(
        "Cannot overwrite a zero-length range – use appendLeft or prependRight instead".to_string(),
      );
    }
    if start >= end {
      return Err(format!("end must be greater than start, got start: {start}, end: {end}"));
    }
    self.split_at(start)?;
    self.split_at(end)?;

    let start_idx = self.chunk_by_start.get(&start).copied().unwrap();
    let end_idx = self.chunk_by_end.get(&end).copied().unwrap();

    if start_idx != end_idx {
      // When the update range spans multiple chunks, we need to:
      // 1. Detect if any chunk within the range has been moved (via `move()`). A moved
      //    chunk's linked-list successor (`chunk.next`) will differ from its positional
      //    successor (`chunk_by_start[chunk.end]`). Overwriting across such a boundary
      //    is invalid because the chunks are no longer contiguous in the output.
      // 2. Clear each interior/end chunk's content (set to "").
      //
      // This mirrors the JS magic-string `update()` implementation:
      //   https://github.com/Rich-Harris/magic-string/blob/410fd4d/src/MagicString.js#L420-L428
      let mut chunk_idx = start_idx;
      loop {
        let next_in_list = self.chunks[chunk_idx].next;
        let chunk_end = self.chunks[chunk_idx].end();
        let next_by_position = self.chunk_by_start.get(&chunk_end).copied();

        if next_in_list != next_by_position {
          return Err("Cannot overwrite across a split point".to_string());
        }

        chunk_idx = next_in_list.unwrap();
        // Interior chunks always clear intro/outro (`Default` has `overwrite: true`),
        // matching JS magic-string where `chunk.edit('', false)` passes
        // `contentOnly=undefined` (falsy), so intro/outro are always cleared.
        self.chunks[chunk_idx].edit("".into(), Default::default());

        if chunk_idx == end_idx {
          break;
        }
      }
    }

    // Edit the start chunk last — only this chunk receives the replacement content
    // and respects the caller's `overwrite` option (JS `contentOnly`).
    self.chunks[start_idx]
      .edit(content, EditOptions { overwrite: opts.overwrite, store_name: opts.keep_original });
    Ok(self)
  }
}
