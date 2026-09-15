pub mod append;
pub mod indent;
pub mod movement;
pub mod prepend;
pub mod replace;
pub mod reset;
pub mod slice;
#[cfg(feature = "sourcemap")]
pub mod source_map;
pub mod trim;
pub mod update;

use std::{borrow::Cow, collections::VecDeque, sync::OnceLock};

use rustc_hash::FxHashMap;

use crate::{
  CowStr,
  chunk::{Chunk, ChunkIdx},
  span::Span,
  type_aliases::IndexChunks,
};

#[derive(Debug, Default)]
pub struct MagicStringOptions {
  pub filename: Option<String>,
  pub ignore_list: bool,
}

#[derive(Debug, Clone)]
pub struct MagicString<'s> {
  filename: Option<String>,
  ignore_list: bool,
  intro: VecDeque<CowStr<'s>>,
  outro: VecDeque<CowStr<'s>>,
  source: Cow<'s, str>,
  chunks: IndexChunks<'s>,
  first_chunk_idx: ChunkIdx,
  last_chunk_idx: ChunkIdx,
  chunk_by_start: FxHashMap<u32, ChunkIdx>,
  chunk_by_end: FxHashMap<u32, ChunkIdx>,
  guessed_indentor: OnceLock<String>,

  /// Original text of every range replaced with `keep_original`, mapped to its position in the
  /// generated sourcemap's `names`. Mirrors `magic-string`'s `storedNames`: the name recorded
  /// is the *whole* requested range, independent of how that range happens to be split into
  /// chunks, so a range that spans a split boundary still stores its full original text.
  stored_names: FxHashMap<String, u32>,

  // This is used to speed up the search for the chunk that contains a given index.
  last_searched_chunk_idx: ChunkIdx,
}

impl Default for MagicString<'_> {
  fn default() -> Self {
    MagicString::new("")
  }
}

impl<'text> MagicString<'text> {
  pub fn new(source: impl Into<Cow<'text, str>>) -> Self {
    Self::with_options(source, Default::default())
  }

  pub fn with_options(source: impl Into<Cow<'text, str>>, options: MagicStringOptions) -> Self {
    let source = source.into();
    debug_assert!(
      source.len() <= u32::MAX as usize,
      "MagicString does not support sources larger than 4GB"
    );
    let source_len = source.len() as u32;
    let initial_chunk = Chunk::new(Span(0, source_len));
    let mut chunks = IndexChunks::with_capacity(1);
    let initial_chunk_idx = chunks.push(initial_chunk);
    let mut magic_string = Self {
      intro: Default::default(),
      outro: Default::default(),
      source,
      first_chunk_idx: initial_chunk_idx,
      last_chunk_idx: initial_chunk_idx,
      chunks,
      chunk_by_start: Default::default(),
      chunk_by_end: Default::default(),
      filename: options.filename,
      ignore_list: options.ignore_list,
      guessed_indentor: OnceLock::default(),
      stored_names: FxHashMap::default(),
      last_searched_chunk_idx: initial_chunk_idx,
    };

    magic_string.chunk_by_start.insert(0, initial_chunk_idx);
    magic_string.chunk_by_end.insert(source_len, initial_chunk_idx);

    magic_string
  }

  pub fn source(&self) -> &str {
    &self.source
  }

  pub fn filename(&self) -> Option<&str> {
    self.filename.as_deref()
  }

  pub fn ignore_list(&self) -> bool {
    self.ignore_list
  }

  /// Returns the length in UTF-8 bytes of the content within chunks (intro + content + outro
  /// per chunk), excluding the global intro/outro from `prepend`/`append`.
  /// This aligns with the reference `magic-string` behavior.
  ///
  /// Note this counts *bytes*. Callers that need the length JavaScript would report (UTF-16
  /// code units, as the reference `magic-string` returns) must use [`Self::len_utf16`].
  pub fn len(&self) -> usize {
    self.iter_chunks().flat_map(|c| c.fragments(&self.source)).map(|f| f.len()).sum()
  }

  /// Returns the length in UTF-16 code units of the content within chunks, excluding the
  /// global intro/outro — the same range [`Self::len`] covers.
  ///
  /// This is what the reference `magic-string` `length()` returns, since JavaScript string
  /// length is measured in UTF-16 code units.
  pub fn len_utf16(&self) -> usize {
    self
      .iter_chunks()
      .flat_map(|c| c.fragments(&self.source))
      .map(|f| f.chars().map(char::len_utf16).sum::<usize>())
      .sum()
  }

  /// Returns the length in UTF-8 bytes of the whole generated output, including the global
  /// intro/outro. Equivalent to `self.to_string().len()` without the allocation.
  fn output_len(&self) -> usize {
    self.intro.iter().map(|f| f.len()).sum::<usize>()
      + self.len()
      + self.outro.iter().map(|f| f.len()).sum::<usize>()
  }

  /// Returns `true` if all chunk content (intro + content + outro) is whitespace or empty.
  /// This aligns with the reference `magic-string` behavior where `isEmpty()` uses `.trim()`.
  pub fn is_empty(&self) -> bool {
    self.iter_chunks().flat_map(|c| c.fragments(&self.source)).all(|f| f.trim().is_empty())
  }

  /// Indicates if the string has been changed.
  ///
  /// The length check is only a fast path for "definitely changed", so it has to measure the
  /// same span as the string comparison below it: the whole output, global intro/outro
  /// included. Comparing against [`Self::len`] instead would report a change for edits that
  /// cancel out, e.g. `remove(0, 1)` followed by `prepend("a")`.
  pub fn has_changed(&self) -> bool {
    self.output_len() != self.source.len() || self.source.as_ref() != self.to_string()
  }

  /// Returns the last character of the generated string, or `None` if empty.
  pub fn last_char(&self) -> Option<char> {
    // Check outro first (last in output order)
    if let Some(last_outro) = self.outro.back()
      && let Some(c) = last_outro.chars().next_back()
    {
      return Some(c);
    }

    // Traverse chunks from last to first
    let mut chunk_idx = Some(self.last_chunk_idx);
    while let Some(idx) = chunk_idx {
      let chunk = &self.chunks[idx];

      // Check chunk outro
      if let Some(last_outro) = chunk.outro.back()
        && let Some(c) = last_outro.chars().next_back()
      {
        return Some(c);
      }

      // Check chunk content (edited or original)
      let content = chunk
        .edited_content
        .as_ref()
        .map(|s| s.as_ref())
        .unwrap_or_else(|| chunk.span.text(&self.source));
      if let Some(c) = content.chars().next_back() {
        return Some(c);
      }

      // Check chunk intro
      if let Some(last_intro) = chunk.intro.back()
        && let Some(c) = last_intro.chars().next_back()
      {
        return Some(c);
      }

      chunk_idx = chunk.prev;
    }

    // Check intro last (first in output order, but we're going backwards)
    if let Some(last_intro) = self.intro.back()
      && let Some(c) = last_intro.chars().next_back()
    {
      return Some(c);
    }

    None
  }

  /// Returns the content after the last newline in the generated string.
  pub fn last_line(&self) -> String {
    // Scan sections from the back — outro, chunks last-to-first, then intro — stopping at
    // the first newline that completes the last line; earlier sections are never touched.
    let mut pieces: Vec<&str> = Vec::new();
    'done: {
      if Self::scan_last_line(self.outro.iter().rev().map(|s| s.as_ref()), &mut pieces) {
        break 'done;
      }
      let mut chunk_idx = Some(self.last_chunk_idx);
      while let Some(idx) = chunk_idx {
        let chunk = &self.chunks[idx];
        let content = chunk
          .edited_content
          .as_ref()
          .map(|s| s.as_ref())
          .unwrap_or_else(|| chunk.span.text(&self.source));
        let fragments = (chunk.outro.iter().rev().map(|s| s.as_ref()))
          .chain(std::iter::once(content))
          .chain(chunk.intro.iter().rev().map(|s| s.as_ref()));
        if Self::scan_last_line(fragments, &mut pieces) {
          break 'done;
        }
        chunk_idx = chunk.prev;
      }
      Self::scan_last_line(self.intro.iter().rev().map(|s| s.as_ref()), &mut pieces);
    }
    let mut last_line = String::with_capacity(pieces.iter().map(|piece| piece.len()).sum());
    pieces.iter().rev().for_each(|piece| last_line.push_str(piece));
    last_line
  }

  /// Pushes fragments (in reverse output order) onto `pieces`, stopping at and returning
  /// `true` for the first one that contains a newline (which completes the last line).
  fn scan_last_line<'a>(
    fragments: impl Iterator<Item = &'a str>,
    pieces: &mut Vec<&'a str>,
  ) -> bool {
    for fragment in fragments {
      let newline = memchr::memrchr(b'\n', fragment.as_bytes());
      pieces.push(&fragment[newline.map_or(0, |index| index + 1)..]);
      if newline.is_some() {
        return true;
      }
    }
    false
  }

  fn prepend_intro(&mut self, content: impl Into<CowStr<'text>>) {
    self.intro.push_front(content.into());
  }

  fn append_outro(&mut self, content: impl Into<CowStr<'text>>) {
    self.outro.push_back(content.into());
  }

  pub fn prepend_outro(&mut self, content: impl Into<CowStr<'text>>) {
    self.outro.push_front(content.into());
  }

  pub fn append_intro(&mut self, content: impl Into<CowStr<'text>>) {
    self.intro.push_back(content.into());
  }

  /// Records `source[start..end)` as a sourcemap name, keeping first-insertion order.
  /// See [`Self::stored_names`].
  pub(super) fn store_name(&mut self, start: u32, end: u32) {
    let original = &self.source[start as usize..end as usize];
    if !self.stored_names.contains_key(original) {
      #[expect(clippy::cast_possible_truncation, reason = "a source has < u32::MAX edits")]
      let next_id = self.stored_names.len() as u32;
      self.stored_names.insert(original.to_string(), next_id);
    }
  }

  /// Stored names in `names`-array order, paired with their index.
  pub(super) fn stored_names_ordered(&self) -> Vec<(&str, u32)> {
    let mut ordered: Vec<(&str, u32)> =
      self.stored_names.iter().map(|(name, &id)| (name.as_str(), id)).collect();
    ordered.sort_unstable_by_key(|&(_, id)| id);
    ordered
  }

  fn iter_chunks(&self) -> impl Iterator<Item = &Chunk<'_>> {
    IterChunks { next: Some(self.first_chunk_idx), chunks: &self.chunks }
  }

  /// Returns `true` if the byte range `[start, end)` lies within a single chunk that has
  /// already been edited. Callers use this for split points that cannot be expressed as a
  /// byte index (a UTF-16 position inside a surrogate pair): the split would land strictly
  /// inside the character spanning `[start, end)`, so it hits an edited chunk exactly when
  /// this returns `true`.
  pub fn is_range_within_edited_chunk(&self, start: u32, end: u32) -> bool {
    self.iter_chunks().any(|c| c.start() <= start && end <= c.end() && c.is_edited())
  }

  pub(crate) fn fragments(&'text self) -> impl Iterator<Item = &'text str> {
    let intro = self.intro.iter().map(|s| s.as_ref());
    let outro = self.outro.iter().map(|s| s.as_ref());
    let chunks = self.iter_chunks().flat_map(|c| c.fragments(&self.source));
    intro.chain(chunks).chain(outro)
  }

  /// For input
  /// "abcdefg"
  ///  0123456
  ///
  /// Chunk{span: (0, 7)} => "abcdefg"
  ///
  /// split_at(3) would create
  ///
  /// Chunk{span: (0, 3)} => "abc"
  /// Chunk{span: (3, 7)} => "defg"
  fn split_at(&mut self, at_index: u32) -> Result<(), String> {
    if at_index == 0
      || (at_index as usize) >= self.source.len()
      || self.chunk_by_end.contains_key(&at_index)
    {
      return Ok(());
    }

    let (mut candidate, mut candidate_idx, search_right) = {
      let last_searched_chunk = &self.chunks[self.last_searched_chunk_idx];
      let search_right = at_index > last_searched_chunk.end();

      (last_searched_chunk, self.last_searched_chunk_idx, search_right)
    };

    while !candidate.contains(at_index) {
      let next_idx = if search_right {
        self.chunk_by_start[&candidate.end()]
      } else {
        self.chunk_by_end[&candidate.start()]
      };
      candidate = &self.chunks[next_idx];
      candidate_idx = next_idx;
    }

    let second_half_chunk = self.chunks[candidate_idx].split(at_index)?;
    let second_half_span = second_half_chunk.span;
    let second_half_idx = self.chunks.push(second_half_chunk);
    let first_half_idx = candidate_idx;

    // Update the last searched chunk
    self.last_searched_chunk_idx = first_half_idx;

    // Update the chunk_by_start/end maps
    self.chunk_by_end.insert(at_index, first_half_idx);
    self.chunk_by_start.insert(at_index, second_half_idx);
    self.chunk_by_end.insert(second_half_span.end(), second_half_idx);

    // Make sure the new chunk and the old chunk have correct next/prev pointers
    self.chunks[second_half_idx].next = self.chunks[first_half_idx].next;
    if let Some(second_half_next_idx) = self.chunks[second_half_idx].next {
      self.chunks[second_half_next_idx].prev = Some(second_half_idx);
    }
    self.chunks[second_half_idx].prev = Some(first_half_idx);
    self.chunks[first_half_idx].next = Some(second_half_idx);
    if first_half_idx == self.last_chunk_idx {
      self.last_chunk_idx = second_half_idx
    }
    Ok(())
  }

  /// Returns the chunk starting at `text_index`, or `None` if no chunk does — in which case
  /// the caller sends the content to the global outro.
  ///
  /// Do not short-circuit on `text_index == self.source.len()`: for an empty source the sole
  /// `[0, 0)` chunk both starts *and* ends at 0, so index 0 has a chunk even though it is also
  /// the end of the source. The map lookup already answers this correctly for both cases, and
  /// matches the reference `magic-string`, which does a plain `byStart[index]` lookup.
  fn by_start_mut(&mut self, text_index: u32) -> Result<Option<&mut Chunk<'text>>, String> {
    self.split_at(text_index)?;
    let idx = self.chunk_by_start.get(&text_index);
    Ok(idx.map(|idx| &mut self.chunks[*idx]))
  }

  /// Returns the chunk ending at `text_index`, or `None` if no chunk does — in which case the
  /// caller sends the content to the global intro.
  ///
  /// Do not short-circuit on `text_index == 0`: see [`Self::by_start_mut`].
  fn by_end_mut(&mut self, text_index: u32) -> Result<Option<&mut Chunk<'text>>, String> {
    self.split_at(text_index)?;
    let idx = self.chunk_by_end.get(&text_index);
    Ok(idx.map(|idx| &mut self.chunks[*idx]))
  }
}

#[expect(clippy::to_string_trait_impl)] // `impl Display` causes extra allocation
impl ToString for MagicString<'_> {
  fn to_string(&self) -> String {
    let size_hint = self.fragments().map(|f| f.len()).sum();
    let mut ret = String::with_capacity(size_hint);
    self.fragments().for_each(|f| ret.push_str(f));
    ret
  }
}

struct IterChunks<'a> {
  next: Option<ChunkIdx>,
  chunks: &'a IndexChunks<'a>,
}

impl<'a> Iterator for IterChunks<'a> {
  type Item = &'a Chunk<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    self.next.map(|idx| {
      let chunk = &self.chunks[idx];
      self.next = chunk.next;
      chunk
    })
  }
}
