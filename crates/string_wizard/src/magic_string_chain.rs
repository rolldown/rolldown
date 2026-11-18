use std::cell::OnceCell;

use rustc_hash::FxHashMap;

use crate::{MagicString, chunk::Chunk};

/// The token that the map has at the start of an edited segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mapping {
  /// The byte offset in the original source that the token points to.
  original: u32,
  /// The index of the token's name in `MagicStringChain::names`.
  name: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
  output_start: u32,
  output_end: u32,
  /// The original range that the segment covers: the same text for an identity segment, the
  /// union of the replaced ranges for an edit, and an empty range for text with no token, at the
  /// insertion point of inserted text. The map uses it only for an identity segment.
  original_start: u32,
  original_end: u32,
  /// The map has one token for the segment, at its start: an edit, or a part of an edit or of
  /// inserted text that a later step carried forward (see `carry_forward_tokens`).
  edited: bool,
  /// The one token of an edited segment, at its start. It points where the map of the earlier
  /// steps points at that position, which is not always the start of the covered range: an
  /// earlier step can move code. `None` means that the token has no original position.
  mapping: Option<Mapping>,
}

impl Segment {
  fn inserted(output_start: u32, len: u32, anchor: u32) -> Self {
    Self {
      output_start,
      output_end: output_start + len,
      original_start: anchor,
      original_end: anchor,
      edited: false,
      mapping: None,
    }
  }

  /// Text with no token in the map: inserted text in the step that inserted it, or a part of an
  /// edit or of inserted text that a later step carried forward and that starts with a '\n'.
  fn is_inserted(&self) -> bool {
    !self.edited && self.original_start == self.original_end
  }

  /// 1:1 byte mapping — positions within can be linearly mapped to original.
  fn is_identity(&self) -> bool {
    !self.edited && self.original_start != self.original_end
  }
}

/// Chains multiple MagicString transformations while maintaining a position
/// mapping from the current output back to the original source.
///
/// This allows generating a single sourcemap at the end without intermediate
/// sourcemap generation and composition, which is more efficient.
///
/// The token of an edit points where the map of the earlier steps points at the start of the
/// edit, as when each step generates a map and the traced collapse composes the maps. The tokens
/// of that map depend on `hires`, so set it with `with_hires` before the first step.
///
/// In text that a step does not edit, the map of the step can have fewer tokens than the map of
/// the earlier steps. With `hires: false`, it has one token for each line of an unedited chunk,
/// and with `hires: boundary`, one token for each word. Thus the composed maps can point to the
/// start of the line or of the word. The chain keeps the tokens of the earlier steps there.
///
/// # Example
/// ```
/// use string_wizard::MagicStringChain;
///
/// let mut sc = MagicStringChain::new("const a = 1;");
/// let mut s = sc.start();
/// s.update(10, 11, "100").unwrap();
/// sc.end(s);
/// assert_eq!(sc.current_output(), "const a = 100;");
///
/// let mut s = sc.start();
/// s.update(13, 14, "").unwrap();
/// sc.end(s);
/// assert_eq!(sc.current_output(), "const a = 100");
/// ```
pub struct MagicStringChain {
  /// The original source string.
  original_source: String,
  /// The current output string (after all steps so far).
  current_output: String,
  /// Position mapping: each segment maps an output range to an original range.
  /// Segments are in output order. They cover the output with no gap, and no segment is empty.
  segments: Vec<Segment>,
  /// The names of the tokens of edited segments.
  names: Vec<String>,
  /// Whether a step was composed. Before the first step, a position maps to itself.
  has_steps: bool,
  /// Whether any step MagicString had `ignore_list` set.
  ignore_list: bool,
  /// The `hires` of the map. A step looks up its edits in the map of the earlier steps, and the
  /// tokens of an unedited segment in that map depend on it.
  #[cfg(feature = "sourcemap")]
  hires: crate::Hires,
}

impl MagicStringChain {
  /// Create a new chain with the given original source.
  pub fn new(source: impl Into<String>) -> Self {
    let source = source.into();
    let len = source.len() as u32;
    let segments = if len > 0 {
      vec![Segment {
        output_start: 0,
        output_end: len,
        original_start: 0,
        original_end: len,
        edited: false,
        mapping: None,
      }]
    } else {
      vec![]
    };
    Self {
      current_output: source.clone(),
      original_source: source,
      segments,
      names: Vec::new(),
      has_steps: false,
      ignore_list: false,
      #[cfg(feature = "sourcemap")]
      hires: crate::Hires::default(),
    }
  }

  /// Sets the `hires` of the map that [`Self::source_map`] generates. Set it before the first
  /// step, because each step looks up its edits in the map of the earlier steps.
  #[cfg(feature = "sourcemap")]
  #[must_use]
  pub fn with_hires(mut self, hires: crate::Hires) -> Self {
    self.hires = hires;
    self
  }

  /// Start a new transformation step.
  /// Returns a fresh MagicString wrapping the current output.
  pub fn start(&self) -> MagicString<'static> {
    MagicString::new(self.current_output.clone())
  }

  /// End a transformation step.
  /// Composes the MagicString's changes with the existing position mapping.
  ///
  /// # Panics
  /// Panics if the MagicString's source doesn't match the chain's current output.
  /// This indicates that a non-chained transformation happened in between.
  pub fn end(&mut self, s: MagicString<'_>) {
    debug_assert_eq!(
      self.current_output,
      s.source(),
      "MagicString source must match chain's current output. \
       A non-chained transformation may have happened between start() and end()."
    );
    if s.ignore_list() {
      self.ignore_list = true;
    }
    let (new_segments, new_names) = self.compose_segments(&s);
    self.names.extend(new_names.into_iter().map(String::from));
    self.current_output = s.to_string();
    self.segments = merge_segments(&self.current_output, new_segments);
    self.has_steps = true;
  }

  /// Returns true if the given source matches the chain's current output.
  /// Use this to check whether a MagicString can be composed into this chain.
  pub fn is_source_compatible(&self, source: &str) -> bool {
    self.current_output == source
  }

  /// Returns a reference to the current output string.
  pub fn current_output(&self) -> &str {
    &self.current_output
  }

  /// Returns a reference to the original source string.
  pub fn original_source(&self) -> &str {
    &self.original_source
  }

  /// Generate a sourcemap from the original source to the current output.
  ///
  /// Builds the sourcemap directly from the composed segments, which correctly
  /// handles relocations, the names that steps store with `keep_original`, and `ignore_list`.
  #[cfg(feature = "sourcemap")]
  pub fn source_map(&self, opts: crate::SourceMapOptions) -> oxc_sourcemap::SourceMap<'static> {
    use crate::{
      source_map::{locator::Locator, sourcemap_builder::SourcemapBuilder},
      span::Span,
    };

    debug_assert_eq!(
      opts.hires, self.hires,
      "The steps looked up their edits with the `hires` of `with_hires`, so the map must use it."
    );
    let mut builder = SourcemapBuilder::new(opts.hires);
    builder.set_source_and_content(&opts.source, &self.original_source);

    let locator = Locator::new(&self.original_source);
    let utf16_map = crate::source_map::precompute_utf16_index_map(
      &self.original_source,
      self.segments.iter().filter_map(|segment| {
        if segment.edited {
          segment.mapping.map(|mapping| mapping.original)
        } else {
          segment.is_identity().then_some(segment.original_start)
        }
      }),
    );

    for segment in &self.segments {
      let content =
        &self.current_output[segment.output_start as usize..segment.output_end as usize];

      if segment.edited {
        if let Some(mapping) = segment.mapping {
          // Build a temporary edited chunk so we can reuse SourcemapBuilder::add_chunk.
          let mut chunk = Chunk::new(Span(mapping.original, mapping.original));
          chunk.edited_content = Some(content.into());
          let name_id = mapping.name.map(|id| builder.add_name(&self.names[id as usize]));
          builder.add_chunk(
            &chunk,
            utf16_map[&mapping.original],
            &locator,
            &self.original_source,
            name_id,
          );
        } else {
          // The edit has no original position. Its token ends the mapping before it on the line,
          // as the traced collapse ends it.
          builder.add_unmapped(content);
        }
      } else if segment.is_inserted() {
        // Inserted content — advance output position, no source token.
        builder.advance(content);
      } else {
        // Identity — unedited chunk: per-character tokens.
        let chunk = Chunk::new(Span(segment.original_start, segment.original_end));
        builder.add_chunk(
          &chunk,
          utf16_map[&segment.original_start],
          &locator,
          &self.original_source,
          None,
        );
      }
    }

    let mut source_map = builder.into_source_map();
    if self.ignore_list {
      source_map.set_ignore_list(vec![0]);
    }
    source_map
  }

  /// Compose the step MagicString's changes with the existing position mapping.
  /// Returns the new segments and the names that they add to `names`.
  fn compose_segments<'s>(&self, s: &'s MagicString<'_>) -> (Vec<Segment>, Vec<&'s str>) {
    let layout = Layout::new(&self.current_output);
    let mut names = StepNames::new(self.names.len() as u32);
    let mut new_segments = Vec::new();
    let mut new_output_pos = 0u32;

    // Global intro (from prepend) — inserted content
    for intro in s.global_intro() {
      let len = intro.len() as u32;
      if len > 0 {
        new_segments.push(Segment::inserted(new_output_pos, len, 0));
        new_output_pos += len;
      }
    }

    for chunk in s.iter_chunks() {
      // Chunk intro (from append_right / prepend_right)
      for intro_frag in &chunk.intro {
        let len = intro_frag.len() as u32;
        if len > 0 {
          let anchor = self.map_output_position(chunk.start());
          new_segments.push(Segment::inserted(new_output_pos, len, anchor));
          new_output_pos += len;
        }
      }

      let chunk_start = chunk.start();
      let chunk_end = chunk.end();

      if let Some(ref edited_content) = chunk.edited_content {
        let content_len = edited_content.len() as u32;
        // A removal has no token in the map of its step, so it needs no segment.
        if content_len > 0 {
          let (original_start, original_end) = self.map_output_range(chunk_start, chunk_end);
          // An edit has one token, even when it writes the original text again, as in the map
          // of its step. The token points where the map of the earlier steps points.
          let mapping = self.trace(chunk_start, &layout).map(|traced| Mapping {
            original: traced.original,
            // The name of the earliest step wins, as in the traced collapse.
            name: traced.name.or_else(|| names.of(s, chunk)),
          });
          new_segments.push(Segment {
            output_start: new_output_pos,
            output_end: new_output_pos + content_len,
            original_start,
            original_end,
            edited: true,
            mapping,
          });
          new_output_pos += content_len;
        }
      } else {
        self.carry_forward_segments(
          chunk_start,
          chunk_end,
          &layout,
          &mut new_output_pos,
          &mut new_segments,
        );
      }

      // Chunk outro (from append_left / prepend_left)
      for outro_frag in &chunk.outro {
        let len = outro_frag.len() as u32;
        if len > 0 {
          let anchor = self.map_output_position(chunk.end());
          new_segments.push(Segment::inserted(new_output_pos, len, anchor));
          new_output_pos += len;
        }
      }
    }

    // Global outro (from append) — inserted content
    for outro in s.global_outro() {
      let len = outro.len() as u32;
      if len > 0 {
        let source_len = self.original_source.len() as u32;
        new_segments.push(Segment::inserted(new_output_pos, len, source_len));
        new_output_pos += len;
      }
    }

    (new_segments, names.names)
  }

  /// The token that the map of the steps so far has at the output position `pos`. The traced
  /// collapse looks up each token of a step's map in the map of the step before it with
  /// `lookup_token_approx`, so this finds the same token: the token at `pos`, else the last token
  /// before `pos` on its line, else the first token of the line. Returns `None` if the line has no
  /// token, or if the token has no original position.
  fn trace(&self, pos: u32, layout: &Layout<'_>) -> Option<Mapping> {
    if !self.has_steps {
      return Some(Mapping { original: pos, name: None });
    }
    let (line_start, line_end) = layout.line_of(pos);
    // The segment at `index` contains `pos`. The segments before it end at or before `pos`.
    // `merge_segments` merges adjacent inserted segments, so each loop skips at most one segment
    // in a row.
    let index = self.segments.partition_point(|segment| segment.output_end <= pos);
    let (before, after) = self.segments.split_at((index + 1).min(self.segments.len()));

    for segment in before.iter().rev() {
      if segment.output_end <= line_start {
        break;
      }
      if segment.is_inserted() {
        continue;
      }
      if segment.edited {
        if segment.output_start < line_start {
          // Its token is on an earlier line.
          break;
        }
        return segment.mapping;
      }
      // The map has a token at `first`: it is the segment start or the line start.
      let first = segment.output_start.max(line_start);
      let last = if segment.output_end <= pos {
        prev_char_start(&self.current_output, segment.output_end)
      } else if self.current_output.as_bytes()[pos as usize] != b'\n' {
        pos
      } else if first < pos {
        prev_char_start(&self.current_output, pos)
      } else {
        continue;
      };
      let token = self.last_identity_token(first, last, layout);
      return Some(Mapping {
        original: segment.original_start + (token - segment.output_start),
        name: None,
      });
    }

    // The line has no token at or before `pos`, so the first token of the line. An edit that
    // starts at the '\n' that ends the line has its token on the line.
    for segment in after {
      if segment.output_start > line_end {
        break;
      }
      if segment.is_inserted() {
        continue;
      }
      if segment.edited {
        return segment.mapping;
      }
      if segment.output_start == line_end {
        // An identity segment has no token at a '\n'.
        break;
      }
      return Some(Mapping { original: segment.original_start, name: None });
    }
    None
  }

  /// The last token in `[first, last]` of an identity segment. The map has a token at `first`,
  /// `last` is the start of a char, and the range has no line break. See
  /// `SourcemapBuilder::add_chunk` for the tokens of each `hires`.
  #[cfg(feature = "sourcemap")]
  fn last_identity_token(&self, first: u32, last: u32, layout: &Layout<'_>) -> u32 {
    use crate::Hires;
    match self.hires {
      Hires::False => first,
      Hires::True => last,
      Hires::Boundary => layout.word_start(first, last),
    }
  }

  #[cfg(not(feature = "sourcemap"))]
  fn last_identity_token(&self, first: u32, _last: u32, _layout: &Layout<'_>) -> u32 {
    first
  }

  /// Map a single output position to the corresponding original position.
  fn map_output_position(&self, pos: u32) -> u32 {
    let index = self.segments.partition_point(|segment| segment.output_end <= pos);
    match self.segments.get(index) {
      Some(segment) if segment.is_identity() => {
        segment.original_start + (pos - segment.output_start)
      }
      Some(segment) => segment.original_start,
      // pos is at or past the end of all segments
      None => self.original_source.len() as u32,
    }
  }

  /// The segments that overlap the output range [start, end).
  fn segments_in(&self, start: u32, end: u32) -> impl Iterator<Item = &Segment> {
    let first = self.segments.partition_point(|segment| segment.output_end <= start);
    self.segments[first..].iter().take_while(move |segment| segment.output_start < end)
  }

  /// Map an output range [start, end) to the corresponding original range.
  /// Returns the union of original ranges for all overlapping segments.
  fn map_output_range(&self, start: u32, end: u32) -> (u32, u32) {
    if start == end {
      let pos = self.map_output_position(start);
      return (pos, pos);
    }

    let mut orig_start = u32::MAX;
    let mut orig_end = 0u32;

    for seg in self.segments_in(start, end) {
      if seg.is_identity() {
        // Identity: linear mapping for the overlapping part
        let overlap_start = start.max(seg.output_start);
        let overlap_end = end.min(seg.output_end);
        orig_start = orig_start.min(seg.original_start + (overlap_start - seg.output_start));
        orig_end = orig_end.max(seg.original_start + (overlap_end - seg.output_start));
      } else {
        // Edited or inserted: use the full original range
        orig_start = orig_start.min(seg.original_start);
        orig_end = orig_end.max(seg.original_end);
      }
    }

    if orig_start == u32::MAX {
      let source_len = self.original_source.len() as u32;
      (source_len, source_len)
    } else {
      (orig_start, orig_end)
    }
  }

  /// Carry forward existing segments for an unedited chunk in the step MagicString.
  /// The chunk range [chunk_start, chunk_end) is in current_output positions.
  fn carry_forward_segments(
    &self,
    chunk_start: u32,
    chunk_end: u32,
    layout: &Layout<'_>,
    new_output_pos: &mut u32,
    new_segments: &mut Vec<Segment>,
  ) {
    for seg in self.segments_in(chunk_start, chunk_end) {
      let overlap_start = chunk_start.max(seg.output_start);
      let overlap_end = chunk_end.min(seg.output_end);
      let overlap_len = overlap_end - overlap_start;
      if overlap_len == 0 {
        continue;
      }

      let output_start = *new_output_pos;
      let output_end = output_start + overlap_len;
      *new_output_pos = output_end;
      if seg.is_identity() {
        let original_start = seg.original_start + (overlap_start - seg.output_start);
        new_segments.push(Segment {
          output_start,
          output_end,
          original_start,
          original_end: original_start + overlap_len,
          edited: false,
          mapping: None,
        });
      } else {
        self.carry_forward_tokens(
          seg,
          (overlap_start, overlap_end),
          output_start,
          layout,
          new_segments,
        );
      }
    }
  }

  /// Carries forward the piece `[start, end)` of an edited or inserted segment. The piece starts
  /// at `output_start` in the output of this step.
  ///
  /// The map of this step has tokens in the piece, as in all text that the step does not edit,
  /// but no token at a '\n'. The map of the earlier steps has at most one token in the segment,
  /// at its start. So each token in the piece points where the map of the earlier steps points at
  /// the start of the token's line in the piece. Thus the piece gets a token at its start and a
  /// token at the start of its last line, and the lines between them have no token in either map.
  /// A part that starts with a '\n' has no token on its first line, so it gets no token.
  fn carry_forward_tokens(
    &self,
    seg: &Segment,
    (start, end): (u32, u32),
    output_start: u32,
    layout: &Layout<'_>,
    new_segments: &mut Vec<Segment>,
  ) {
    let text = self.current_output.as_bytes();
    let last_line = memchr::memrchr(b'\n', &text[start as usize..end as usize])
      .map_or(start, |i| start + i as u32 + 1);
    for (part_start, part_end) in [(start, last_line), (last_line, end)] {
      if part_start == part_end {
        continue;
      }
      let part_output_start = output_start + (part_start - start);
      let len = part_end - part_start;
      new_segments.push(if text[part_start as usize] == b'\n' {
        Segment::inserted(part_output_start, len, seg.original_start)
      } else {
        // The token of an edit at the start of the edit stays.
        let mapping = if seg.edited && part_start == seg.output_start {
          seg.mapping
        } else {
          self.trace(part_start, layout)
        };
        Segment {
          output_start: part_output_start,
          output_end: part_output_start + len,
          edited: true,
          mapping,
          ..*seg
        }
      });
    }
  }
}

impl std::fmt::Display for MagicStringChain {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.current_output)
  }
}

/// The source of a step, with what the lookups of the step use again: the line breaks, found on
/// first use, and the long runs of word chars that `word_start` walked.
struct Layout<'a> {
  text: &'a str,
  breaks: OnceCell<Vec<u32>>,
  /// Each run as start → end. A start is the start of a word in `text`, so two runs never
  /// overlap, and a word has at most one run.
  #[cfg(feature = "sourcemap")]
  words: std::cell::RefCell<std::collections::BTreeMap<u32, u32>>,
}

impl<'a> Layout<'a> {
  fn new(text: &'a str) -> Self {
    Self {
      text,
      breaks: OnceCell::new(),
      #[cfg(feature = "sourcemap")]
      words: std::cell::RefCell::default(),
    }
  }

  /// The token of the char at `pos` in an identity segment with `hires: boundary`: `pos` for a
  /// char that is not a word char, else the start of its word, but not before `first`.
  ///
  /// A walk back of `RECORD_FROM` bytes or more records its run, and a later walk stops at the
  /// end of a run. So the lookups of a step cost linear time, also when the edits of a step that
  /// moves code alternate between long words. A shorter walk costs less than a record.
  #[cfg(feature = "sourcemap")]
  fn word_start(&self, first: u32, pos: u32) -> u32 {
    const RECORD_FROM: u32 = 64;
    let c = self.text[pos as usize..].chars().next().expect("a char starts at `pos`");
    if !is_word(c) {
      return pos;
    }
    let end = pos + c.len_utf8() as u32;
    let mut words = self.words.borrow_mut();
    let mut start = pos;
    // Only this run can end in the word before `pos`, because the runs do not overlap.
    if let Some((&run_start, run_end)) = words.range_mut(..=pos).next_back() {
      if pos >= *run_end {
        start = walk_back_words(self.text, pos, *run_end);
      }
      if start <= *run_end {
        // `pos` is in the run, or its word continues into the run.
        *run_end = (*run_end).max(end);
        return run_start.max(first);
      }
    } else {
      start = walk_back_words(self.text, pos, 0);
    }
    if pos - start >= RECORD_FROM {
      words.insert(start, end);
    }
    start.max(first)
  }

  /// The line of `pos`: its start, and the position of the '\n' that ends it (or the text end).
  fn line_of(&self, pos: u32) -> (u32, u32) {
    let breaks = self
      .breaks
      .get_or_init(|| memchr::memchr_iter(b'\n', self.text.as_bytes()).map(|i| i as u32).collect());
    let index = breaks.partition_point(|&line_break| line_break < pos);
    let start = index.checked_sub(1).map_or(0, |i| breaks[i] + 1);
    let end = breaks.get(index).copied().unwrap_or(self.text.len() as u32);
    (start, end)
  }
}

/// The names that one step adds to `MagicStringChain::names`.
struct StepNames<'s> {
  first_id: u32,
  ids: FxHashMap<&'s str, u32>,
  names: Vec<&'s str>,
}

impl<'s> StepNames<'s> {
  fn new(first_id: u32) -> Self {
    Self { first_id, ids: FxHashMap::default(), names: Vec::new() }
  }

  /// The name of an edited chunk in the map of its own step: the text that the edit replaced, if
  /// the step stored it with `keep_original`.
  fn of(&mut self, s: &'s MagicString<'_>, chunk: &Chunk<'_>) -> Option<u32> {
    if !chunk.keep_in_mappings {
      return None;
    }
    let text = chunk.span.text(s.source());
    if !s.is_stored_name(text) {
      return None;
    }
    let next_id = self.first_id + self.names.len() as u32;
    Some(*self.ids.entry(text).or_insert_with(|| {
      self.names.push(text);
      next_id
    }))
  }
}

/// The start of the char that ends at `pos`.
fn prev_char_start(text: &str, pos: u32) -> u32 {
  let c = text[..pos as usize].chars().next_back().expect("a char ends at `pos`");
  pos - c.len_utf8() as u32
}

/// A word char of `hires: boundary`. See `SourcemapBuilder::add_chunk`.
#[cfg(feature = "sourcemap")]
fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// The start of the word chars that end at `pos`, but not before `stop`.
#[cfg(feature = "sourcemap")]
fn walk_back_words(text: &str, mut pos: u32, stop: u32) -> u32 {
  while pos > stop {
    match text[..pos as usize].chars().next_back() {
      Some(c) if is_word(c) => pos -= c.len_utf8() as u32,
      _ => break,
    }
  }
  pos
}

/// Merges a segment into the segment before it when the map has the same tokens for the two
/// segments as for one segment. The lookups of the next step then skip each run at once.
/// - Two inserted segments have no token. The first anchor stays.
/// - Two edited segments have the same token, and the first one has no line break. A lookup in
///   the second one then finds a token with the same mapping on the same line. This joins the
///   pieces of an edit that a later step splits.
fn merge_segments(output: &str, segments: Vec<Segment>) -> Vec<Segment> {
  let has_line_break = |segment: &Segment| {
    let content = &output.as_bytes()[segment.output_start as usize..segment.output_end as usize];
    memchr::memchr(b'\n', content).is_some()
  };
  let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
  // Whether the last merged segment has a line break. Each segment is scanned once, so a long
  // line of merged pieces costs linear time.
  let mut last_has_line_break = None;

  for seg in segments {
    if let Some(last) = merged.last_mut() {
      if last.is_inserted() && seg.is_inserted() {
        last.output_end = seg.output_end;
        continue;
      }
      if last.edited
        && seg.edited
        && last.mapping == seg.mapping
        && !*last_has_line_break.get_or_insert_with(|| has_line_break(last))
      {
        last.output_end = seg.output_end;
        last.original_start = last.original_start.min(seg.original_start);
        last.original_end = last.original_end.max(seg.original_end);
        last_has_line_break = Some(has_line_break(&seg));
        continue;
      }
    }
    merged.push(seg);
    last_has_line_break = None;
  }

  merged
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic_single_step() {
    let mut sc = MagicStringChain::new("const a = 1;");
    let mut s = sc.start();
    s.update(10, 11, "100").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "const a = 100;");
  }

  #[test]
  fn issue_example_two_steps() {
    // The exact example from the issue
    let mut sc = MagicStringChain::new("const a = 1;");

    let mut s = sc.start();
    s.update(10, 11, "100").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "const a = 100;");

    let mut s = sc.start();
    // Position 13 in "const a = 100;" is the ";", position 14 is end
    s.update(13, 14, "").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "const a = 100");
  }

  #[test]
  fn two_steps_non_overlapping() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.update(0, 2, "XY").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "XYcdef");

    let mut s = sc.start();
    s.update(4, 6, "ZZ").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "XYcdZZ");
  }

  #[test]
  fn second_step_at_boundary_of_first_edit() {
    let mut sc = MagicStringChain::new("abcdef");

    // Step 1: replace "cd" with "XYZ"
    let mut s = sc.start();
    s.update(2, 4, "XYZ").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abXYZef");

    // Step 2: edit right after the first edit (position 5 = "e")
    let mut s = sc.start();
    s.update(5, 6, "Q").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abXYZQf");
  }

  #[test]
  fn second_step_overwrites_entire_previous_edit() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.update(2, 4, "XYZ").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abXYZef");

    // Step 2: overwrite the entire previous edit region [2, 5) = "XYZ"
    let mut s = sc.start();
    s.update(2, 5, "Q").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abQef");
  }

  #[test]
  fn edit_spanning_previous_edit_and_unedited() {
    let mut sc = MagicStringChain::new("abcdefg");

    // Step 1: replace "de" with "xyz"
    let mut s = sc.start();
    s.update(3, 5, "xyz").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abcxyzfg");

    // Step 2: overwrite across the edit boundary [2,5) = "cxy" → "Q"
    let mut s = sc.start();
    s.update(2, 5, "Q").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abQzfg");
  }

  #[test]
  fn removal() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.remove(2, 4).unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abef");

    // Step 2: operate on the shorter string
    let mut s = sc.start();
    s.update(0, 2, "XY").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "XYef");
  }

  #[test]
  fn append_left_insertion() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.append_left(3, "XY").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abcXYdef");

    // Step 2: edit after the insertion
    let mut s = sc.start();
    s.update(5, 8, "QRS").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abcXYQRS");
  }

  #[test]
  fn global_prepend_and_append() {
    let mut sc = MagicStringChain::new("abc");

    let mut s = sc.start();
    s.prepend(">>>");
    s.append("<<<");
    sc.end(s);
    assert_eq!(sc.current_output(), ">>>abc<<<");

    // Step 2: edit the original content (positions shifted by prepend)
    let mut s = sc.start();
    s.update(3, 6, "XYZ").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), ">>>XYZ<<<");
  }

  #[test]
  fn multiple_edits_in_one_step() {
    let mut sc = MagicStringChain::new("abcdefgh");

    let mut s = sc.start();
    s.update(1, 2, "B").unwrap();
    s.update(5, 6, "F").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "aBcdeFgh");

    let mut s = sc.start();
    s.update(0, 1, "A").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "ABcdeFgh");
  }

  #[test]
  fn three_sequential_steps() {
    let mut sc = MagicStringChain::new("hello world");

    // Step 1
    let mut s = sc.start();
    s.update(0, 5, "HELLO").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "HELLO world");

    // Step 2
    let mut s = sc.start();
    s.update(6, 11, "WORLD").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "HELLO WORLD");

    // Step 3
    let mut s = sc.start();
    s.update(5, 6, "_").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "HELLO_WORLD");
  }

  #[test]
  fn no_modification_step() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.update(2, 4, "XY").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abXYef");

    // Step 2: no modifications
    let s = sc.start();
    sc.end(s);
    assert_eq!(sc.current_output(), "abXYef");
  }

  #[test]
  fn edit_that_grows_then_shrinks() {
    let mut sc = MagicStringChain::new("ab");

    // Step 1: grow "a" into "xyz"
    let mut s = sc.start();
    s.update(0, 1, "xyz").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "xyzb");

    // Step 2: shrink "xyzb" to "Qb" by replacing "xyz" (positions 0-3)
    let mut s = sc.start();
    s.update(0, 3, "Q").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "Qb");
  }

  #[test]
  fn append_right_insertion() {
    let mut sc = MagicStringChain::new("abcdef");

    let mut s = sc.start();
    s.append_right(3, "XY").unwrap();
    sc.end(s);
    assert_eq!(sc.current_output(), "abcXYdef");
  }

  #[test]
  fn chain_with_replace() {
    let mut sc = MagicStringChain::new("hello world hello");

    let mut s = sc.start();
    let _ = s.replace("hello", "hi");
    sc.end(s);
    assert_eq!(sc.current_output(), "hi world hello");

    let mut s = sc.start();
    let _ = s.replace("hello", "hi");
    sc.end(s);
    assert_eq!(sc.current_output(), "hi world hi");
  }

  #[cfg(feature = "sourcemap")]
  mod sourcemap_tests {
    use super::*;
    use crate::SourceMapOptions;
    use crate::source_map::sourcemap_builder::Hires;
    use crate::{MagicStringOptions, UpdateOptions};
    use oxc_sourcemap::SourcemapVisualizer;

    #[test]
    fn sourcemap_basic() {
      let mut sc = MagicStringChain::new("const a = 1;");

      let mut s = sc.start();
      s.update(10, 11, "100").unwrap();
      sc.end(s);

      let mut s = sc.start();
      s.update(13, 14, "").unwrap();
      sc.end(s);

      assert_eq!(sc.current_output(), "const a = 100");

      let sm = sc.source_map(SourceMapOptions {
        source: "test.js".into(),
        include_content: true,
        hires: Hires::False,
      });

      // Verify the sourcemap was generated successfully
      assert!(sm.get_source(0).is_some());
    }

    #[test]
    fn sourcemap_with_insertion() {
      let mut sc = MagicStringChain::new("abcdef");

      let mut s = sc.start();
      s.append_left(3, "XY").unwrap();
      sc.end(s);

      assert_eq!(sc.current_output(), "abcXYdef");

      let sm = sc.source_map(SourceMapOptions {
        source: "test.js".into(),
        include_content: false,
        hires: Hires::False,
      });

      assert!(sm.get_source(0).is_some());
    }

    #[test]
    fn sourcemap_matches_direct_magic_string() {
      // Verify that the chain's sourcemap produces equivalent results
      // to applying all edits directly on a single MagicString
      let mut direct = MagicString::new("const a = 1;".to_string());
      direct.update(10, 11, "100").unwrap();
      direct.remove(11, 12).unwrap();

      let direct_sm = direct.source_map(SourceMapOptions {
        source: "test.js".into(),
        include_content: true,
        hires: Hires::False,
      });

      let mut sc = MagicStringChain::new("const a = 1;");
      let mut s = sc.start();
      s.update(10, 11, "100").unwrap();
      sc.end(s);
      let mut s = sc.start();
      // In the chain output "const a = 100;", position 13 is ";", 14 is end
      s.update(13, 14, "").unwrap();
      sc.end(s);

      let chain_sm = sc.source_map(SourceMapOptions {
        source: "test.js".into(),
        include_content: true,
        hires: Hires::False,
      });

      // Both should produce the same output
      assert_eq!(direct.to_string(), sc.current_output());

      // Compare token counts (both should have same number of source tokens)
      assert_eq!(direct_sm.get_tokens().count(), chain_sm.get_tokens().count());
    }

    #[test]
    fn sourcemap_matches_direct_magic_string_for_relocate() {
      let mut direct = MagicString::new("abcdef".to_string());
      direct.relocate(0, 2, 6).unwrap();

      let mut chain = MagicStringChain::new("abcdef").with_hires(Hires::True);
      let mut step = chain.start();
      step.relocate(0, 2, 6).unwrap();
      chain.end(step);

      let output = direct.to_string();
      assert_eq!(output, chain.current_output());

      let opts =
        || SourceMapOptions { source: "test.js".into(), include_content: true, hires: Hires::True };
      let direct_sm = direct.source_map(opts());
      let chain_sm = chain.source_map(opts());

      assert_eq!(
        SourcemapVisualizer::new(&output, &direct_sm).get_text(),
        SourcemapVisualizer::new(&output, &chain_sm).get_text()
      );
    }

    #[test]
    fn sourcemap_preserves_original_names_from_step() {
      let mut direct = MagicString::new("abc".to_string());
      direct
        .update_with(1, 2, "B", UpdateOptions { keep_original: true, ..Default::default() })
        .unwrap();

      let mut chain = MagicStringChain::new("abc");
      let mut step = chain.start();
      step
        .update_with(1, 2, "B", UpdateOptions { keep_original: true, ..Default::default() })
        .unwrap();
      chain.end(step);

      let direct_sm = direct.source_map(SourceMapOptions::default());
      let chain_sm = chain.source_map(SourceMapOptions::default());
      let direct_names = direct_sm.get_names().collect::<Vec<_>>();
      let chain_names = chain_sm.get_names().collect::<Vec<_>>();

      assert_eq!(direct_names, chain_names);
    }

    #[test]
    fn sourcemap_preserves_ignore_list_from_step() {
      let mut direct = MagicString::with_options(
        "abc".to_string(),
        MagicStringOptions { ignore_list: true, ..Default::default() },
      );
      direct.update(1, 2, "B").unwrap();

      let mut chain = MagicStringChain::new("abc");
      let mut step = MagicString::with_options(
        "abc".to_string(),
        MagicStringOptions { ignore_list: true, ..Default::default() },
      );
      step.update(1, 2, "B").unwrap();
      chain.end(step);

      let direct_sm = direct.source_map(SourceMapOptions::default());
      let chain_sm = chain.source_map(SourceMapOptions::default());

      assert_eq!(direct_sm.get_ignore_list(), chain_sm.get_ignore_list());
    }

    /// When a non-MagicString plugin returns an explicit sourcemap between two
    /// native-MagicString plugins, the pipeline must flush the current chain,
    /// eagerly collapse with the barrier, and start a fresh chain.
    #[test]
    fn sourcemap_chain_flush_and_collapse_at_barrier() {
      use rolldown_sourcemap::collapse_sourcemaps;

      let source = "abcdef";

      let mut first_step = MagicString::new(source.to_string());
      first_step.update(0, 1, "A").unwrap();
      let after_first_step = first_step.to_string();
      let first_step_map = first_step.source_map(SourceMapOptions {
        source: "original.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      // A plugin that returns the same code but a non-identity sourcemap.
      let mut barrier = MagicString::new("uvwxyz".to_string());
      barrier.update(0, 6, after_first_step.clone()).unwrap();
      let barrier_map = barrier.source_map(SourceMapOptions {
        source: "barrier.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      let mut second_step = MagicString::new(after_first_step.clone());
      second_step.update(1, 2, "B").unwrap();
      let output = second_step.to_string();
      let second_step_map = second_step.source_map(SourceMapOptions {
        source: "after-first.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      // Ground truth: collapsing all three individual sourcemaps.
      let expected = collapse_sourcemaps(&[&first_step_map, &barrier_map, &second_step_map]);

      // Simulate the background-thread pipeline:
      // 1. Chain accumulates first_step, then a Barrier arrives.
      let mut chain1 = MagicStringChain::new(source).with_hires(Hires::True);
      chain1.end(first_step);
      let chain1_map = chain1.source_map(SourceMapOptions {
        source: "original.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      // 2. Eagerly collapse the chain's sourcemap with the barrier.
      let base_map = collapse_sourcemaps(&[&chain1_map, &barrier_map]);

      // 3. New chain for the second step.
      let mut chain2 = MagicStringChain::new(after_first_step).with_hires(Hires::True);
      chain2.end(second_step);
      let chain2_map = chain2.source_map(SourceMapOptions {
        source: "after-first.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      // 4. Collapse the accumulated base with the new chain.
      let actual = collapse_sourcemaps(&[&base_map, &chain2_map]);

      assert_eq!(
        SourcemapVisualizer::new(&output, &expected).get_text(),
        SourcemapVisualizer::new(&output, &actual).get_text()
      );
    }

    type Step = fn(&mut MagicString<'_>);

    /// The source, line, column and name that a lookup finds, or `None` for no mapping.
    type Found = Option<(String, u32, u32, Option<String>)>;

    fn map_options(hires: Hires) -> SourceMapOptions {
      SourceMapOptions { source: "test.js".into(), include_content: false, hires }
    }

    fn keep_original() -> UpdateOptions {
      UpdateOptions { keep_original: true, ..Default::default() }
    }

    /// The map without a chain: one map for each step, composed by the traced collapse, as the
    /// sourcemap chain of a module does.
    fn per_step_map(
      source: &str,
      hires: Hires,
      steps: &[Step],
    ) -> (String, oxc_sourcemap::SourceMap<'static>) {
      let mut code = source.to_string();
      let mut maps = Vec::new();
      for step in steps {
        let mut s = MagicString::new(code.clone());
        step(&mut s);
        maps.push(s.source_map(map_options(hires)));
        code = s.to_string();
      }
      let maps = maps.iter().collect::<Vec<_>>();
      (code, rolldown_sourcemap::collapse_sourcemaps_traced(&maps))
    }

    fn chain_map(
      source: &str,
      hires: Hires,
      steps: &[Step],
    ) -> (String, oxc_sourcemap::SourceMap<'static>) {
      let mut chain = MagicStringChain::new(source).with_hires(hires);
      for step in steps {
        let mut s = chain.start();
        step(&mut s);
        chain.end(s);
      }
      (chain.current_output().to_string(), chain.source_map(map_options(hires)))
    }

    /// What a lookup finds at a generated position. With `approx`, a position before the first
    /// token of its line finds that token, as in the lookups of a later collapse.
    fn lookup(map: &oxc_sourcemap::SourceMap<'_>, line: u32, column: u32, approx: bool) -> Found {
      let table = map.generate_lookup_table();
      let token = if approx {
        map.lookup_token_approx(&table, line, column)
      } else {
        map.lookup_token(&table, line, column)
      }?;
      let source = map.get_source(token.get_source_id()?)?.to_string();
      let name = token.get_name_id().and_then(|id| map.get_name(id)).map(ToString::to_string);
      Some((source, token.get_src_line(), token.get_src_col(), name))
    }

    fn found(line: u32, column: u32, name: Option<&str>) -> Found {
      Some(("test.js".to_string(), line, column, name.map(ToString::to_string)))
    }

    /// Checks that the chain map and the per-step map find the same original position and name
    /// at each generated position (0-based line and column), with the exact lookup and with the
    /// approximate lookup. Returns what the exact lookup in the chain map finds.
    fn assert_same_lookups(
      source: &str,
      hires: Hires,
      steps: &[Step],
      positions: &[(u32, u32)],
    ) -> Vec<Found> {
      let (expected_code, expected_map) = per_step_map(source, hires, steps);
      let (code, map) = chain_map(source, hires, steps);
      assert_eq!(code, expected_code);
      positions
        .iter()
        .map(|&(line, column)| {
          for approx in [false, true] {
            let actual = lookup(&map, line, column, approx);
            let expected = lookup(&expected_map, line, column, approx);
            assert_eq!(
              actual, expected,
              "{hires:?}, approx {approx}: generated {line}:{column} of {code:?}"
            );
          }
          lookup(&map, line, column, false)
        })
        .collect()
    }

    /// Each column of each line of `code`, and the column after the end of the line.
    fn every_position(code: &str) -> Vec<(u32, u32)> {
      code
        .split('\n')
        .enumerate()
        .flat_map(|(line, text)| (0..=text.len() as u32).map(move |column| (line as u32, column)))
        .collect()
    }

    /// Checks the lookups at each position of the output.
    fn assert_same_lookups_everywhere(source: &str, hires: Hires, steps: &[Step]) {
      let (code, _) = per_step_map(source, hires, steps);
      assert_same_lookups(source, hires, steps, &every_position(&code));
    }

    #[test]
    fn edit_over_moved_code_points_where_its_start_was() {
      // Move line 2 before line 1, then replace both lines with one edit.
      let steps: [Step; 2] = [
        |s| {
          s.relocate(9, 18, 0).unwrap();
        },
        |s| {
          s.update(0, 17, "replaced()").unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        let found_at = assert_same_lookups("line1();\nline2();\n", hires, &steps, &[(0, 0)]);
        assert_eq!(found_at, [found(1, 0, None)]);
      }
    }

    #[test]
    fn edits_of_moved_lines_point_to_their_lines() {
      let steps: [Step; 2] = [
        |s| {
          s.relocate(9, 18, 0).unwrap();
        },
        |s| {
          s.update(0, 5, "LINE2").unwrap();
          s.update(9, 14, "LINE1").unwrap();
        },
      ];
      let found_at =
        assert_same_lookups("line1();\nline2();\n", Hires::Boundary, &steps, &[(0, 0), (1, 0)]);
      assert_eq!(found_at, [found(1, 0, None), found(0, 0, None)]);
    }

    #[test]
    fn edit_points_where_the_map_of_earlier_steps_points() {
      let steps: [Step; 2] = [
        |s| {
          s.update(10, 11, "100").unwrap();
        },
        |s| {
          s.update(6, 7, "b").unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        // With `hires: false`, the map of the first step has a token only at the line start.
        let column = if hires == Hires::False { 0 } else { 6 };
        let found_at = assert_same_lookups("const a = 1;", hires, &steps, &[(0, 6)]);
        assert_eq!(found_at, [found(0, column, None)]);
      }
    }

    #[test]
    fn edit_in_inserted_text_points_where_the_map_points() {
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        // The insertion has its own line, which has no token.
        let steps: [Step; 2] = [
          |s| {
            s.prepend("// banner\n");
          },
          |s| {
            s.update(3, 9, "BANNER").unwrap();
          },
        ];
        assert_eq!(assert_same_lookups("foo();\n", hires, &steps, &[(0, 3)]), [None]);

        // The insertion is in a line: the last token before the edit.
        let steps: [Step; 2] = [
          |s| {
            s.append_left(3, "XY").unwrap();
          },
          |s| {
            s.update(4, 5, "Q").unwrap();
          },
        ];
        let column = if hires == Hires::True { 2 } else { 0 };
        let found_at = assert_same_lookups("abcdef", hires, &steps, &[(0, 4)]);
        assert_eq!(found_at, [found(0, column, None)]);

        // The insertion is at the line start: the first token of the line.
        let steps: [Step; 2] = [
          |s| {
            s.prepend("XY");
          },
          |s| {
            s.update(0, 2, "Q").unwrap();
          },
        ];
        assert_eq!(assert_same_lookups("abc\n", hires, &steps, &[(0, 0)]), [found(0, 0, None)]);
      }
    }

    #[test]
    fn edit_after_the_first_line_of_an_earlier_edit_has_no_mapping() {
      // The map of the first step has one token for its edit, at the start of line 1.
      let steps: [Step; 2] = [
        |s| {
          s.update(0, 10, "x();\ny();\n").unwrap();
        },
        |s| {
          s.update(5, 6, "z").unwrap();
        },
      ];
      let found_at =
        assert_same_lookups("a();\nb();\n", Hires::Boundary, &steps, &[(0, 0), (1, 0), (1, 2)]);
      assert_eq!(found_at, [found(0, 0, None), None, None]);
    }

    #[test]
    fn name_of_the_earliest_step_wins() {
      let source = "foo();\n";
      // Only the first rename stores a name.
      let steps: [Step; 2] = [
        |s| {
          s.update_with(0, 3, "bar", keep_original()).unwrap();
        },
        |s| {
          s.update(0, 3, "baz").unwrap();
        },
      ];
      let found_at = assert_same_lookups(source, Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(0, 0, Some("foo"))]);

      // Only the second rename stores a name: the text before the second rename.
      let steps: [Step; 2] = [
        |s| {
          s.update(0, 3, "bar").unwrap();
        },
        |s| {
          s.update_with(0, 3, "baz", keep_original()).unwrap();
        },
      ];
      let found_at = assert_same_lookups(source, Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(0, 0, Some("bar"))]);

      // Both renames store a name.
      let steps: [Step; 2] = [
        |s| {
          s.update_with(0, 3, "bar", keep_original()).unwrap();
        },
        |s| {
          s.update_with(0, 3, "baz", keep_original()).unwrap();
        },
      ];
      let found_at = assert_same_lookups(source, Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(0, 0, Some("foo"))]);
    }

    #[test]
    fn edit_with_the_same_text_keeps_its_name() {
      let source = "foo();\n";
      let steps: [Step; 1] = [|s| {
        s.update_with(0, 3, "foo", keep_original()).unwrap();
      }];
      let found_at = assert_same_lookups(source, Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(0, 0, Some("foo"))]);

      let steps: [Step; 2] = [
        |s| {
          s.update(0, 3, "bar").unwrap();
        },
        |s| {
          s.update_with(0, 3, "bar", keep_original()).unwrap();
        },
      ];
      let found_at = assert_same_lookups(source, Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(0, 0, Some("bar"))]);
    }

    #[test]
    fn edit_with_the_same_text_points_where_the_map_points() {
      // Move line 2 before line 1, then write the same text again over both lines.
      let steps: [Step; 2] = [
        |s| {
          s.relocate(5, 10, 0).unwrap();
        },
        |s| {
          s.update(0, 10, "x();\nx();\n").unwrap();
        },
      ];
      let found_at = assert_same_lookups("x();\nx();\n", Hires::Boundary, &steps, &[(0, 0)]);
      assert_eq!(found_at, [found(1, 0, None)]);

      // The edit starts in inserted text on its own line, which has no token.
      let steps: [Step; 2] = [
        |s| {
          s.prepend("X\n");
        },
        |s| {
          s.update(0, 5, "abc").unwrap();
        },
      ];
      assert_eq!(assert_same_lookups("abc", Hires::Boundary, &steps, &[(0, 0)]), [None]);

      // With `hires: false`, the map of the first step has a token only at the line start.
      let steps: [Step; 2] = [
        |s| {
          s.update(10, 11, "100").unwrap();
        },
        |s| {
          s.update(6, 7, "a").unwrap();
        },
      ];
      let found_at = assert_same_lookups("const a = 1;", Hires::False, &steps, &[(0, 6)]);
      assert_eq!(found_at, [found(0, 0, None)]);
    }

    #[test]
    fn edit_with_no_mapping_ends_the_mapping_before_it() {
      // Join the line of the inserted text to the line before it, then edit the inserted text.
      let steps: [Step; 2] = [
        |s| {
          s.append("// banner");
        },
        |s| {
          s.remove(6, 7).unwrap();
          s.update(10, 16, "BANNER").unwrap();
        },
      ];
      let found_at =
        assert_same_lookups("foo();\n", Hires::Boundary, &steps, &[(0, 5), (0, 7), (0, 9)]);
      assert_eq!(found_at, [found(0, 5, None), None, None]);
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_same_lookups_everywhere("foo();\n", hires, &steps);
      }
    }

    #[test]
    fn edit_before_an_edit_at_a_line_break_points_where_the_map_points() {
      // The token of the first edit is at the '\n' that ends the first line.
      let steps: [Step; 2] = [
        |s| {
          s.update(0, 2, "\nB").unwrap();
          s.prepend("XY");
        },
        |s| {
          s.update(0, 1, "Z").unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_eq!(assert_same_lookups("\nb", hires, &steps, &[(0, 0)]), [found(0, 0, None)]);
      }
    }

    #[test]
    fn inserted_text_that_a_step_joins_to_the_line_before_has_no_mapping() {
      let steps: [Step; 2] = [
        |s| {
          s.append("\nbar();");
        },
        |s| {
          s.remove(6, 7).unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        // With `hires: false`, the map has a token only at the line start.
        let column = if hires == Hires::False { 0 } else { 5 };
        let found_at = assert_same_lookups("foo();", hires, &steps, &[(0, 5), (0, 6), (0, 9)]);
        assert_eq!(found_at, [found(0, column, None), None, None]);
      }
    }

    #[test]
    fn edit_that_a_step_carries_forward_has_no_token_at_a_line_break() {
      // The map of the second step has no token at the '\n' of the edit.
      let steps: [Step; 2] = [
        |s| {
          s.update(0, 1, "\n").unwrap();
        },
        |s| {
          s.prepend("X");
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_eq!(assert_same_lookups("\nb", hires, &steps, &[(0, 1)]), [None]);
        assert_same_lookups_everywhere("\nb", hires, &steps);
      }
    }

    #[test]
    fn edit_that_a_step_carries_forward_points_where_the_map_points_on_its_last_line() {
      // On the second line of the edit, the map of the first step has only the tokens of `();`.
      let steps: [Step; 2] = [
        |s| {
          s.update(0, 6, "x();\ny").unwrap();
        },
        |s| {
          s.prepend("// ");
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        let found_at = assert_same_lookups("a();\nb();", hires, &steps, &[(1, 0)]);
        assert_eq!(found_at, [found(1, 1, None)]);
        assert_same_lookups_everywhere("a();\nb();", hires, &steps);
      }
    }

    #[test]
    fn inserted_text_that_a_step_carries_forward_points_where_the_map_points() {
      // A step with no edits. The first line of the inserted text has no token, and its last line
      // gets the first token of the line.
      let steps: [Step; 2] = [
        |s| {
          s.prepend("X\nY");
        },
        |_| {},
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        let found_at = assert_same_lookups("abc", hires, &steps, &[(0, 0), (1, 0)]);
        assert_eq!(found_at, [None, found(0, 0, None)]);
        assert_same_lookups_everywhere("abc", hires, &steps);
      }
    }

    #[test]
    fn edits_in_one_long_word_point_where_the_per_step_maps_point() {
      let steps: [Step; 2] = [
        |s| {
          s.append(";");
        },
        |s| {
          s.replace_all("a", "b").unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_same_lookups_everywhere(&"a".repeat(200), hires, &steps);
      }
    }

    #[test]
    fn moved_edits_that_alternate_between_two_words_point_where_the_per_step_maps_point() {
      // Step 2 moves each `b` after the `a` with the same index, and edits both. It looks up its
      // edits in output order, so the lookups alternate between the two words. The words are
      // longer than `RECORD_FROM`, so the lookups record their runs. The step also edits the
      // space, because text that a step does not edit can keep a token. See `MagicStringChain`.
      let steps: [Step; 2] = [
        |s| {
          s.append(";");
        },
        |s| {
          for i in 0..100 {
            s.relocate(101 + i, 102 + i, i + 1).unwrap();
            s.update(i, i + 1, "A").unwrap();
            s.update(101 + i, 102 + i, "B").unwrap();
          }
          s.update(100, 101, " ").unwrap();
        },
      ];
      let source = "a".repeat(100) + " " + &"b".repeat(100);
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_same_lookups_everywhere(&source, hires, &steps);
      }
    }

    #[test]
    fn moved_edits_in_the_two_parts_of_a_word_point_where_the_per_step_maps_point() {
      // Step 1 edits the char in the middle of a word, so the word is in two identity segments:
      // 49 chars before the edit and 50 chars after it. Each `é` has 2 bytes. Step 2 moves each
      // char of one part next to the char of the other part with the same index, and edits all
      // of them. So its lookups alternate between the two parts, in each of the two orders, and
      // they record one run for the two parts.
      fn edit_both_parts(s: &mut MagicString<'_>) {
        for i in 0..50 {
          if i < 49 {
            s.update(2 * i, 2 * i + 2, "A").unwrap();
          }
          s.update(99 + 2 * i, 101 + 2 * i, "B").unwrap();
        }
      }
      let first_part_first: [Step; 2] = [
        |s| {
          s.update(98, 100, "Z").unwrap();
        },
        |s| {
          for i in 0..49 {
            s.relocate(99 + 2 * i, 101 + 2 * i, 2 * i + 2).unwrap();
          }
          edit_both_parts(s);
        },
      ];
      let second_part_first: [Step; 2] = [
        |s| {
          s.update(98, 100, "Z").unwrap();
        },
        |s| {
          for i in 0..49 {
            s.relocate(2 * i, 2 * i + 2, 101 + 2 * i).unwrap();
          }
          edit_both_parts(s);
        },
      ];
      let source = "é".repeat(100);
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        for steps in [&first_part_first, &second_part_first] {
          assert_same_lookups_everywhere(&source, hires, steps);
        }
      }
    }

    #[test]
    fn edits_in_adjacent_inserted_text_point_where_the_per_step_maps_point() {
      let steps: [Step; 2] = [
        |s| {
          for _ in 0..200 {
            s.append_right(0, "x;").unwrap();
          }
        },
        |s| {
          s.replace_all("x", "y").unwrap();
        },
      ];
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        assert_same_lookups_everywhere("foo();\n", hires, &steps);
      }
    }

    #[test]
    fn many_replacements_point_where_the_per_step_maps_point() {
      let steps: [Step; 5] = [
        |s| {
          s.replace_all("foo", "bar").unwrap();
        },
        |s| {
          s.replace_all("bar", "baz").unwrap();
        },
        |s| {
          s.replace_all("baz", "qux").unwrap();
        },
        |s| {
          s.replace_all("qux", "quux").unwrap();
        },
        |s| {
          s.replace_all("quux", "foo").unwrap();
        },
      ];
      let lines = (0..50).map(|i| format!("x{i} = foo;\n")).collect::<String>();
      let one_line = (0..50).map(|i| format!("x{i}=foo;")).collect::<String>();
      for hires in [Hires::False, Hires::True, Hires::Boundary] {
        for source in [&lines, &one_line] {
          assert_same_lookups_everywhere(source, hires, &steps);
        }
      }
    }
  }
}
