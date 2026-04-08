use crate::MagicString;

#[derive(Debug, Clone, Copy)]
struct Segment {
  output_start: u32,
  output_end: u32,
  original_start: u32,
  original_end: u32,
  /// Content was modified (not a 1:1 byte mapping to original).
  edited: bool,
  keep_in_mappings: bool,
}

impl Segment {
  fn is_inserted(&self) -> bool {
    self.original_start == self.original_end
  }

  /// 1:1 byte mapping — positions within can be linearly mapped to original.
  fn is_identity(&self) -> bool {
    !self.is_inserted() && !self.edited
  }
}

/// Chains multiple MagicString transformations while maintaining a position
/// mapping from the current output back to the original source.
///
/// This allows generating a single sourcemap at the end without intermediate
/// sourcemap generation and composition, which is more efficient.
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
  /// Segments are in output order.
  segments: Vec<Segment>,
  /// Whether any step MagicString had `ignore_list` set.
  ignore_list: bool,
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
        keep_in_mappings: false,
      }]
    } else {
      vec![]
    };
    Self { current_output: source.clone(), original_source: source, segments, ignore_list: false }
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
    let new_segments = self.compose_segments(&s);
    self.current_output = s.to_string();
    self.segments = merge_overlapping_segments(new_segments);
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
  /// handles relocations, `keep_in_mappings` names, and `ignore_list`.
  #[cfg(feature = "sourcemap")]
  pub fn source_map(&self, opts: crate::SourceMapOptions) -> oxc_sourcemap::SourceMap {
    use crate::source_map::{locator::Locator, sourcemap_builder::SourcemapBuilder};

    let mut builder = SourcemapBuilder::new(opts.hires);
    builder.set_source_and_content(&opts.source, &self.original_source);

    let locator = Locator::new(&self.original_source);
    let utf16_map = crate::source_map::precompute_utf16_index_map(
      &self.original_source,
      self.segments.iter().filter(|s| !s.is_inserted()).map(|s| s.original_start),
    );

    for segment in &self.segments {
      let content =
        &self.current_output[segment.output_start as usize..segment.output_end as usize];

      if segment.is_inserted() {
        // Inserted content — advance output position, no source token.
        builder.advance(content);
      } else {
        let original_content =
          &self.original_source[segment.original_start as usize..segment.original_end as usize];
        let is_edited = content != original_content;

        if is_edited {
          // Build a temporary edited chunk so we can reuse SourcemapBuilder::add_chunk.
          let mut chunk = crate::chunk::Chunk::new(crate::span::Span(
            segment.original_start,
            segment.original_end,
          ));
          chunk.edited_content = Some(content.into());
          chunk.keep_in_mappings = segment.keep_in_mappings;
          let name = segment
            .keep_in_mappings
            .then(|| &self.original_source[segment.original_start as usize..segment.original_end as usize]);
          builder.add_chunk(
            &chunk,
            utf16_map[&segment.original_start],
            &locator,
            &self.original_source,
            name,
          );
        } else {
          // Identity — unedited chunk: per-character tokens.
          let chunk = crate::chunk::Chunk::new(crate::span::Span(
            segment.original_start,
            segment.original_end,
          ));
          builder.add_chunk(
            &chunk,
            utf16_map[&segment.original_start],
            &locator,
            &self.original_source,
            None,
          );
        }
      }
    }

    let mut source_map = builder.into_source_map();
    if self.ignore_list {
      source_map.set_x_google_ignore_list(vec![0]);
    }
    source_map
  }

  /// Compose the step MagicString's changes with the existing position mapping.
  fn compose_segments(&self, s: &MagicString<'_>) -> Vec<Segment> {
    let mut new_segments = Vec::new();
    let mut new_output_pos = 0u32;

    // Global intro (from prepend) — inserted content
    for intro in s.global_intro() {
      let len = intro.len() as u32;
      if len > 0 {
        new_segments.push(Segment {
          output_start: new_output_pos,
          output_end: new_output_pos + len,
          original_start: 0,
          original_end: 0,
          edited: false,
          keep_in_mappings: false,
        });
        new_output_pos += len;
      }
    }

    for chunk in s.iter_chunks() {
      // Chunk intro (from append_right / prepend_right)
      for intro_frag in &chunk.intro {
        let len = intro_frag.len() as u32;
        if len > 0 {
          let orig_pos = self.map_output_position(chunk.start());
          new_segments.push(Segment {
            output_start: new_output_pos,
            output_end: new_output_pos + len,
            original_start: orig_pos,
            original_end: orig_pos,
            edited: false,
            keep_in_mappings: false,
          });
          new_output_pos += len;
        }
      }

      let chunk_start = chunk.start();
      let chunk_end = chunk.end();

      if let Some(ref edited_content) = chunk.edited_content {
        let (orig_start, orig_end) = self.map_output_range(chunk_start, chunk_end);
        let content_len = edited_content.len() as u32;

        if content_len > 0 || orig_start < orig_end {
          new_segments.push(Segment {
            output_start: new_output_pos,
            output_end: new_output_pos + content_len,
            original_start: orig_start,
            original_end: orig_end,
            edited: true,
            keep_in_mappings: chunk.keep_in_mappings,
          });
        }
        new_output_pos += content_len;
      } else {
        self.carry_forward_segments(chunk_start, chunk_end, &mut new_output_pos, &mut new_segments);
      }

      // Chunk outro (from append_left / prepend_left)
      for outro_frag in &chunk.outro {
        let len = outro_frag.len() as u32;
        if len > 0 {
          let orig_pos = self.map_output_position(chunk.end());
          new_segments.push(Segment {
            output_start: new_output_pos,
            output_end: new_output_pos + len,
            original_start: orig_pos,
            original_end: orig_pos,
            edited: false,
            keep_in_mappings: false,
          });
          new_output_pos += len;
        }
      }
    }

    // Global outro (from append) — inserted content
    for outro in s.global_outro() {
      let len = outro.len() as u32;
      if len > 0 {
        let source_len = self.original_source.len() as u32;
        new_segments.push(Segment {
          output_start: new_output_pos,
          output_end: new_output_pos + len,
          original_start: source_len,
          original_end: source_len,
          edited: false,
          keep_in_mappings: false,
        });
        new_output_pos += len;
      }
    }

    new_segments
  }

  /// Map a single output position to the corresponding original position.
  fn map_output_position(&self, pos: u32) -> u32 {
    for seg in &self.segments {
      if pos >= seg.output_start && pos < seg.output_end {
        if seg.is_identity() {
          return seg.original_start + (pos - seg.output_start);
        }
        return seg.original_start;
      }
    }
    // pos is at or past the end of all segments
    self.original_source.len() as u32
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

    for seg in &self.segments {
      // Skip non-overlapping segments
      if seg.output_end <= start || seg.output_start >= end {
        continue;
      }

      if seg.is_inserted() {
        orig_start = orig_start.min(seg.original_start);
        orig_end = orig_end.max(seg.original_end);
        continue;
      }

      let overlap_start = start.max(seg.output_start);
      let overlap_end = end.min(seg.output_end);

      if seg.is_identity() {
        // Identity: linear mapping for the overlapping part
        let mapped_start = seg.original_start + (overlap_start - seg.output_start);
        let mapped_end = seg.original_start + (overlap_end - seg.output_start);
        orig_start = orig_start.min(mapped_start);
        orig_end = orig_end.max(mapped_end);
      } else {
        // Edited: use the full original range
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
    new_output_pos: &mut u32,
    new_segments: &mut Vec<Segment>,
  ) {
    for seg in &self.segments {
      // Skip non-overlapping segments
      if seg.output_end <= chunk_start || seg.output_start >= chunk_end {
        continue;
      }

      let overlap_start = chunk_start.max(seg.output_start);
      let overlap_end = chunk_end.min(seg.output_end);
      let overlap_len = overlap_end - overlap_start;

      if overlap_len == 0 && !seg.is_inserted() {
        continue;
      }

      if seg.is_inserted() {
        if overlap_len > 0 {
          new_segments.push(Segment {
            output_start: *new_output_pos,
            output_end: *new_output_pos + overlap_len,
            original_start: seg.original_start,
            original_end: seg.original_end,
            edited: seg.edited,
            keep_in_mappings: seg.keep_in_mappings,
          });
          *new_output_pos += overlap_len;
        }
      } else if seg.is_identity() {
        let orig_offset_start = overlap_start - seg.output_start;
        let orig_offset_end = overlap_end - seg.output_start;
        new_segments.push(Segment {
          output_start: *new_output_pos,
          output_end: *new_output_pos + overlap_len,
          original_start: seg.original_start + orig_offset_start,
          original_end: seg.original_start + orig_offset_end,
          edited: false,
          keep_in_mappings: seg.keep_in_mappings,
        });
        *new_output_pos += overlap_len;
      } else {
        // Edited segment: carry forward with full original range
        new_segments.push(Segment {
          output_start: *new_output_pos,
          output_end: *new_output_pos + overlap_len,
          original_start: seg.original_start,
          original_end: seg.original_end,
          edited: true,
          keep_in_mappings: seg.keep_in_mappings,
        });
        *new_output_pos += overlap_len;
      }
    }
  }
}

impl std::fmt::Display for MagicStringChain {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.current_output)
  }
}

/// Merge segments with overlapping original ranges.
///
/// After composition, a new edit can partially consume a previously-edited segment,
/// leaving the remainder with an overlapping original range. This function merges
/// such segments so that sourcemap generation produces correct, non-conflicting edits.
///
/// Only merges when the overlap is genuine (same direction), NOT when segments are
/// simply out of order due to relocations.
fn merge_overlapping_segments(segments: Vec<Segment>) -> Vec<Segment> {
  if segments.len() <= 1 {
    return segments;
  }

  let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());

  for seg in segments {
    let should_merge = merged.last().is_some_and(|last: &Segment| {
      // Merge if both are non-inserted, in the same monotonic direction,
      // and their original ranges genuinely overlap (not merely reordered).
      !last.is_inserted()
        && !seg.is_inserted()
        && seg.original_start >= last.original_start
        && seg.original_start < last.original_end
    });

    if should_merge {
      let last = merged.last_mut().unwrap();
      last.output_end = seg.output_end;
      last.original_end = last.original_end.max(seg.original_end);
      // Preserve keep_in_mappings if either segment has it
      last.keep_in_mappings = last.keep_in_mappings || seg.keep_in_mappings;
    } else {
      merged.push(seg);
    }
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
    s.append_left(3, "XY");
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
    s.append_right(3, "XY");
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
      s.append_left(3, "XY");
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

      let mut chain = MagicStringChain::new("abcdef");
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
      let direct_names = direct_sm.get_names().map(|name| name.as_ref()).collect::<Vec<_>>();
      let chain_names = chain_sm.get_names().map(|name| name.as_ref()).collect::<Vec<_>>();

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

      assert_eq!(direct_sm.get_x_google_ignore_list(), chain_sm.get_x_google_ignore_list());
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
      let mut chain1 = MagicStringChain::new(source);
      chain1.end(first_step);
      let chain1_map = chain1.source_map(SourceMapOptions {
        source: "original.js".into(),
        include_content: true,
        hires: Hires::True,
      });

      // 2. Eagerly collapse the chain's sourcemap with the barrier.
      let base_map = collapse_sourcemaps(&[&chain1_map, &barrier_map]);

      // 3. New chain for the second step.
      let mut chain2 = MagicStringChain::new(after_first_step);
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
  }
}
