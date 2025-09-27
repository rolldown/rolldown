pub mod append;
pub mod indent;
pub mod movement;
pub mod prepend;
pub mod replace;
#[cfg(feature = "sourcemap")]
pub mod source_map;
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
}

#[derive(Debug, Clone)]
pub struct MagicString<'s> {
  filename: Option<String>,
  intro: VecDeque<CowStr<'s>>,
  outro: VecDeque<CowStr<'s>>,
  source: Cow<'s, str>,
  chunks: IndexChunks<'s>,
  first_chunk_idx: ChunkIdx,
  last_chunk_idx: ChunkIdx,
  chunk_by_start: FxHashMap<usize, ChunkIdx>,
  chunk_by_end: FxHashMap<usize, ChunkIdx>,
  guessed_indentor: OnceLock<String>,

  // This is used to speed up the search for the chunk that contains a given index.
  last_searched_chunk_idx: ChunkIdx,
}

impl<'text> MagicString<'text> {
  pub fn new(source: impl Into<Cow<'text, str>>) -> Self {
    Self::with_options(source, Default::default())
  }

  pub fn with_options(source: impl Into<Cow<'text, str>>, options: MagicStringOptions) -> Self {
    let source = source.into();
    let source_len = source.len();
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
      guessed_indentor: OnceLock::default(),
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

  pub fn len(&self) -> usize {
    self.fragments().map(|f| f.len()).sum()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates if the string has been changed.
  pub fn has_changed(&self) -> bool {
    self.source.len() != self.len() || self.source.as_ref() != self.to_string()
  }

  fn prepend_intro(&mut self, content: impl Into<CowStr<'text>>) {
    self.intro.push_front(content.into());
  }

  fn append_outro(&mut self, content: impl Into<CowStr<'text>>) {
    self.outro.push_back(content.into());
  }

  fn prepend_outro(&mut self, content: impl Into<CowStr<'text>>) {
    self.outro.push_front(content.into());
  }

  fn append_intro(&mut self, content: impl Into<CowStr<'text>>) {
    self.intro.push_back(content.into());
  }

  fn iter_chunks(&self) -> impl Iterator<Item = &Chunk<'_>> {
    IterChunks { next: Some(self.first_chunk_idx), chunks: &self.chunks }
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
  fn split_at(&mut self, at_index: usize) {
    if at_index == 0 || at_index >= self.source.len() || self.chunk_by_end.contains_key(&at_index) {
      return;
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

    let second_half_chunk = self.chunks[candidate_idx].split(at_index);
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
  }

  fn by_start_mut(&mut self, text_index: usize) -> Option<&mut Chunk<'text>> {
    if text_index == self.source.len() {
      None
    } else {
      self.split_at(text_index);
      let idx = self.chunk_by_start.get(&text_index)?;
      self.chunks.get_mut(*idx)
    }
  }

  fn by_end_mut(&mut self, text_index: usize) -> Option<&mut Chunk<'text>> {
    if text_index == 0 {
      None
    } else {
      self.split_at(text_index);
      let idx = self.chunk_by_end.get(&text_index)?;
      self.chunks.get_mut(*idx)
    }
  }
}

#[expect(clippy::to_string_trait_impl)] // `impl Display` causes extra allocation
impl ToString for MagicString<'_> {
  fn to_string(&self) -> String {
    let size_hint = self.len();
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
