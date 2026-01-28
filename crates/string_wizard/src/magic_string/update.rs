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

    let start_chunk = &mut self.chunks[start_idx];
    start_chunk
      .edit(content, EditOptions { overwrite: opts.overwrite, store_name: opts.keep_original });

    let mut rest_chunk_idx = if start_idx != end_idx {
      start_chunk.next.unwrap()
    } else {
      return Ok(self);
    };

    loop {
      let rest_chunk = &mut self.chunks[rest_chunk_idx];
      rest_chunk.edit("".into(), Default::default());
      if rest_chunk_idx == end_idx {
        break;
      }
      rest_chunk_idx = rest_chunk.next.unwrap();
    }
    Ok(self)
  }
}
