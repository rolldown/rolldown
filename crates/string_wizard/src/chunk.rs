use std::collections::VecDeque;

use crate::{CowStr, span::Span};

oxc_index::define_index_type! {
    pub struct ChunkIdx = u32;
}

#[derive(Debug)]
pub struct EditOptions {
  /// `true` will clear the `intro` and `outro` of the [Chunk]
  pub overwrite: bool,
  pub store_name: bool,
}

impl Default for EditOptions {
  fn default() -> Self {
    Self { overwrite: true, store_name: false }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Chunk<'str> {
  pub intro: VecDeque<CowStr<'str>>,
  pub outro: VecDeque<CowStr<'str>>,
  pub span: Span,
  pub edited_content: Option<CowStr<'str>>,
  pub next: Option<ChunkIdx>,
  pub prev: Option<ChunkIdx>,
  pub keep_in_mappings: bool,
}

impl Chunk<'_> {
  pub fn new(span: Span) -> Self {
    Self { span, ..Default::default() }
  }
}

impl<'str> Chunk<'str> {
  pub fn start(&self) -> usize {
    self.span.start()
  }

  pub fn end(&self) -> usize {
    self.span.end()
  }

  pub fn contains(&self, text_index: usize) -> bool {
    self.start() < text_index && text_index < self.end()
  }

  pub fn append_outro(&mut self, content: CowStr<'str>) {
    self.outro.push_back(content)
  }

  pub fn append_intro(&mut self, content: CowStr<'str>) {
    self.intro.push_back(content)
  }

  pub fn prepend_outro(&mut self, content: CowStr<'str>) {
    self.outro.push_front(content)
  }

  pub fn prepend_intro(&mut self, content: CowStr<'str>) {
    self.intro.push_front(content)
  }

  pub fn split<'a>(&'a mut self, text_index: usize) -> Chunk<'str> {
    if !(text_index > self.start() && text_index < self.end()) {
      panic!("Cannot split chunk at {text_index} between {:?}", self.span);
    }
    if self.edited_content.is_some() {
      panic!("Cannot split a chunk that has already been edited")
    }
    let first_half_slice = Span(self.start(), text_index);
    let second_half_slice = Span(text_index, self.end());
    let mut new_chunk = Chunk::new(second_half_slice);
    if self.is_edited() {
      new_chunk
        .edit("".into(), EditOptions { store_name: self.keep_in_mappings, overwrite: false });
    }
    std::mem::swap(&mut new_chunk.outro, &mut self.outro);
    self.span = first_half_slice;
    new_chunk
  }

  pub fn fragments(&'str self, original_source: &'str str) -> impl Iterator<Item = &'str str> {
    let intro_iter = self.intro.iter().map(|frag| frag.as_ref());
    let source_frag = self
      .edited_content
      .as_ref()
      .map(|s| s.as_ref())
      .unwrap_or_else(|| self.span.text(original_source));
    let outro_iter = self.outro.iter().map(|frag| frag.as_ref());
    intro_iter.chain(Some(source_frag)).chain(outro_iter)
  }

  pub fn edit(&mut self, content: CowStr<'str>, opts: EditOptions) {
    if opts.overwrite {
      self.intro.clear();
      self.outro.clear();
    }
    self.keep_in_mappings = opts.store_name;
    self.edited_content = Some(content);
  }

  pub fn is_edited(&self) -> bool {
    self.edited_content.is_some()
  }
}
