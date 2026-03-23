#![expect(clippy::inherent_to_string)]
use std::sync::Arc;

use napi::bindgen_prelude::{Either, This};
use napi::{Env, JsString};
use napi_derive::napi;
use rolldown_sourcemap::{JSONSourceMap, SourceMap};
use rolldown_utils::base64::to_standard_base64;
use rolldown_utils::js_regex::HybridRegex;
use serde::Serialize;
use string_wizard::{MagicString, MagicStringOptions, SourceMapOptions, UpdateOptions};

use super::js_regex::JsRegExp;

/// Internal representation preserving the original JS format (flat `[start, end]` vs nested
/// `[[start, end], ...]`) so the getter returns the same shape the user passed in.
#[derive(Clone)]
enum IndentExclusionRanges {
  Flat(Vec<i64>),
  Nested(Vec<Vec<i64>>),
}

impl IndentExclusionRanges {
  fn from_either(either: Either<Vec<Vec<i64>>, Vec<i64>>) -> Self {
    match either {
      Either::A(nested) => Self::Nested(nested),
      Either::B(flat) => Self::Flat(flat),
    }
  }

  fn to_either(&self) -> Either<Vec<Vec<i64>>, Vec<i64>> {
    match self {
      Self::Flat(v) => Either::B(v.clone()),
      Self::Nested(v) => Either::A(v.clone()),
    }
  }
}

/// Normalizes an `Either<Vec<Vec<i64>>, Vec<i64>>` (nested or flat exclusion ranges from JS)
/// into `Vec<(u32, u32)>` byte-offset pairs suitable for the Rust indent implementation.
/// The `offset` is applied to each index before UTF-16→byte conversion, matching the
/// behavior of every other position-based API in this binding.
fn normalize_exclude_ranges(
  ranges: &Either<Vec<Vec<i64>>, Vec<i64>>,
  mapper: &Utf16ToByteMapper,
  offset: i64,
) -> Vec<(u32, u32)> {
  let pairs: Vec<(i64, i64)> = match ranges {
    Either::B(flat) => {
      if flat.len() >= 2 {
        vec![(flat[0], flat[1])]
      } else {
        vec![]
      }
    }
    Either::A(nested) => {
      nested.iter().filter_map(|r| if r.len() >= 2 { Some((r[0], r[1])) } else { None }).collect()
    }
  };

  pairs
    .into_iter()
    .filter_map(|(s, e)| {
      let s_with_offset = u32::try_from(s + offset).ok()?;
      let e_with_offset = u32::try_from(e + offset).ok()?;
      let start = mapper.utf16_to_byte(s_with_offset)?;
      let end = mapper.utf16_to_byte(e_with_offset)?;
      Some((start, end))
    })
    .collect()
}

/// Serializable source map matching the SourceMap V3 specification.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializableSourceMap<'a> {
  version: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<&'a String>,
  sources: &'a Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  sources_content: Option<&'a Vec<Option<String>>>,
  names: &'a Vec<String>,
  mappings: &'a String,
  #[serde(rename = "x_google_ignoreList", skip_serializing_if = "Option::is_none")]
  x_google_ignore_list: Option<&'a Vec<u32>>,
}

/// Per-UTF-16-index mapping entry: byte offset + surrogate code unit.
#[derive(Clone, Copy)]
struct Utf16Mapping {
  /// UTF-8 byte offset at this UTF-16 position.
  byte_offset: u32,
  /// Raw UTF-16 code unit value. 0 for BMP characters and the end sentinel.
  /// High surrogates (0xD800–0xDBFF) and low surrogates (0xDC00–0xDFFF)
  /// store their actual code unit value, used to emit lone surrogates in `slice`.
  surrogate: u16,
}

impl Utf16Mapping {
  #[inline]
  fn is_low_surrogate(self) -> bool {
    (0xDC00..=0xDFFF).contains(&self.surrogate)
  }
}

#[derive(Clone)]
struct Utf16ToByteMapper {
  /// One entry per UTF-16 code unit, plus a sentinel at the end.
  /// Length = utf16_len + 1. Indexed directly by JS string index.
  entries: Vec<Utf16Mapping>,
}

impl Utf16ToByteMapper {
  /// Builds a mapping from UTF-16 code unit positions (JS string indices) to UTF-8 byte offsets.
  ///
  /// JavaScript strings are UTF-16 encoded, so all indices from JS are UTF-16 code unit positions.
  /// Characters outside the BMP (e.g. emoji `🤷`) use 2 UTF-16 code units (a surrogate pair) but
  /// are a single Rust `char`. This mapper accounts for that by pushing one entry per UTF-16 code
  /// unit, so the array is indexed directly by JS string index.
  #[expect(clippy::cast_possible_truncation)]
  fn new(s: &str) -> Self {
    // UTF-16 length <= UTF-8 byte length for all strings, so s.len() + 1
    // is always a valid upper-bound capacity, avoiding a second pass over chars.
    let mut entries = Vec::with_capacity(s.len() + 1);

    let mut byte_offset = 0u32;
    for ch in s.chars() {
      if ch.len_utf16() == 2 {
        let mut buf = [0u16; 2];
        ch.encode_utf16(&mut buf);
        // High surrogate: byte offset *before* the character.
        entries.push(Utf16Mapping { byte_offset, surrogate: buf[0] });
        byte_offset += ch.len_utf8() as u32;
        // Low surrogate: byte offset *after* the character.
        entries.push(Utf16Mapping { byte_offset, surrogate: buf[1] });
      } else {
        entries.push(Utf16Mapping { byte_offset, surrogate: 0 });
        byte_offset += ch.len_utf8() as u32;
      }
    }
    // End sentinel.
    entries.push(Utf16Mapping { byte_offset, surrogate: 0 });

    Self { entries }
  }

  #[inline]
  fn get(&self, utf16_index: u32) -> Option<Utf16Mapping> {
    self.entries.get(utf16_index as usize).copied()
  }

  #[inline]
  fn utf16_to_byte(&self, utf16_offset: u32) -> Option<u32> {
    self.get(utf16_offset).map(|e| e.byte_offset)
  }

  /// Returns the UTF-16 code unit count of the original string.
  /// This matches JavaScript's `String.prototype.length`.
  fn utf16_len(&self) -> i64 {
    #[expect(clippy::cast_possible_wrap)]
    let count = (self.entries.len() - 1) as i64;
    count
  }

  /// Converts a UTF-8 byte offset to a UTF-16 code unit offset.
  /// Returns `None` if the byte offset is past the end of the mapping.
  #[expect(clippy::cast_possible_truncation)]
  fn byte_to_utf16(&self, byte_offset: u32) -> Option<u32> {
    let mut idx = self.entries.partition_point(|e| e.byte_offset < byte_offset);
    // If we landed on a low surrogate, the byte offset is "after" the
    // supplementary character. The correct UTF-16 position is the next
    // index (which shares the same byte_offset).
    if idx < self.entries.len() && self.entries[idx].is_low_surrogate() {
      idx += 1;
    }
    (idx < self.entries.len()).then_some(idx as u32)
  }

  /// Returns the total UTF-8 byte length of the original string.
  /// This is the correct sentinel for out-of-bounds index clamping in `slice`.
  fn total_len(&self) -> u32 {
    self.entries.last().map_or(0, |e| e.byte_offset)
  }

  /// Normalizes a potentially negative index to a positive index.
  /// Negative indices count from the end of the string (matching original magic-string behavior).
  fn normalize_index(&self, index: i64) -> i64 {
    let len = self.utf16_len();
    if len > 0 && index < 0 { ((index % len) + len) % len } else { index }
  }
}

#[napi(object)]
#[derive(Default)]
pub struct BindingMagicStringOptions {
  pub filename: Option<String>,
  pub offset: Option<i64>,
  pub indent_exclusion_ranges: Option<Either<Vec<Vec<i64>>, Vec<i64>>>,
  pub ignore_list: Option<bool>,
}

#[napi(object)]
#[derive(Default)]
pub struct BindingIndentOptions {
  pub exclude: Option<Either<Vec<Vec<i64>>, Vec<i64>>>,
}

#[napi(object)]
#[derive(Default)]
pub struct BindingSourceMapOptions {
  /// The filename for the generated file (goes into `map.file`)
  pub file: Option<String>,
  /// The filename of the original source (goes into `map.sources`)
  pub source: Option<String>,
  pub include_content: Option<bool>,
  /// Accepts boolean or string: true, false, "boundary"
  /// - true: high-resolution sourcemaps (character-level)
  /// - false: low-resolution sourcemaps (line-level) - default
  /// - "boundary": high-resolution only at word boundaries
  pub hires: Option<Either<bool, String>>,
}

/// A source map object with properties matching the SourceMap V3 specification.
#[napi]
pub struct BindingSourceMap {
  json: JSONSourceMap,
}

/// A decoded source map with mappings as an array of arrays instead of VLQ-encoded string.
#[napi]
pub struct BindingDecodedMap {
  inner: SourceMap,
  json: JSONSourceMap,
}

#[napi]
impl BindingSourceMap {
  /// The source map version (always 3).
  #[napi(getter)]
  pub fn version(&self) -> u32 {
    3
  }

  /// The generated file name.
  #[napi(getter)]
  pub fn file(&self) -> Option<String> {
    self.json.file.clone()
  }

  /// The list of original source files.
  #[napi(getter)]
  pub fn sources(&self) -> Vec<String> {
    self.json.sources.clone()
  }

  /// The original source contents (if `includeContent` was true).
  #[napi(getter)]
  pub fn sources_content(&self) -> Vec<Option<String>> {
    self.json.sources_content.clone().unwrap_or_default()
  }

  /// The list of symbol names used in mappings.
  #[napi(getter)]
  pub fn names(&self) -> Vec<String> {
    self.json.names.clone()
  }

  /// The VLQ-encoded mappings string.
  #[napi(getter)]
  pub fn mappings(&self) -> String {
    self.json.mappings.clone()
  }

  /// The list of source indices that should be excluded from debugging.
  #[napi(getter, js_name = "x_google_ignoreList")]
  pub fn x_google_ignore_list(&self) -> Option<Vec<u32>> {
    self.json.x_google_ignore_list.clone()
  }

  /// Returns the source map as a JSON string.
  #[napi]
  pub fn to_string(&self) -> String {
    let serializable = SerializableSourceMap {
      version: 3,
      file: self.json.file.as_ref(),
      sources: &self.json.sources,
      sources_content: self.json.sources_content.as_ref(),
      names: &self.json.names,
      mappings: &self.json.mappings,
      x_google_ignore_list: self.json.x_google_ignore_list.as_ref(),
    };
    serde_json::to_string(&serializable).expect("should be able to serialize source map")
  }

  /// Returns the source map as a base64-encoded data URL.
  #[napi]
  pub fn to_url(&self) -> String {
    let json = self.to_string();
    let base64 = to_standard_base64(&json);
    format!("data:application/json;charset=utf-8;base64,{base64}")
  }
}

#[napi]
impl BindingDecodedMap {
  /// The source map version (always 3).
  #[napi(getter)]
  pub fn version(&self) -> u32 {
    3
  }

  /// The generated file name.
  #[napi(getter)]
  pub fn file(&self) -> Option<String> {
    self.json.file.clone()
  }

  /// The list of original source files.
  #[napi(getter)]
  pub fn sources(&self) -> Vec<String> {
    self.json.sources.clone()
  }

  /// The original source contents (if `includeContent` was true).
  #[napi(getter)]
  pub fn sources_content(&self) -> Vec<Option<String>> {
    self.json.sources_content.clone().unwrap_or_default()
  }

  /// The list of symbol names used in mappings.
  #[napi(getter)]
  pub fn names(&self) -> Vec<String> {
    self.json.names.clone()
  }

  /// The decoded mappings as an array of line arrays.
  /// Each line is an array of segments, where each segment is [generatedColumn, sourceIndex, originalLine, originalColumn, nameIndex?].
  #[napi(getter)]
  pub fn mappings(&self) -> Vec<Vec<Vec<i64>>> {
    let mut lines: Vec<Vec<Vec<i64>>> = Vec::new();

    for token in self.inner.get_tokens() {
      // Fill in empty lines if needed
      while lines.len() <= token.get_dst_line() as usize {
        lines.push(Vec::new());
      }

      let current_line = token.get_dst_line();

      let mut segment: Vec<i64> = vec![i64::from(token.get_dst_col())];

      if let Some(source_id) = token.get_source_id() {
        segment.push(i64::from(source_id));
        segment.push(i64::from(token.get_src_line()));
        segment.push(i64::from(token.get_src_col()));

        if let Some(name_id) = token.get_name_id() {
          segment.push(i64::from(name_id));
        }
      }

      lines[current_line as usize].push(segment);
    }

    lines
  }

  /// The list of source indices that should be excluded from debugging.
  #[napi(getter, js_name = "x_google_ignoreList")]
  pub fn x_google_ignore_list(&self) -> Option<Vec<u32>> {
    self.json.x_google_ignore_list.clone()
  }
}

#[napi]
pub struct BindingMagicString<'a> {
  pub(crate) inner: MagicString<'a>,
  utf16_to_byte_mapper: Utf16ToByteMapper,
  pub(crate) offset: i64,
  indent_exclusion_ranges: Option<IndentExclusionRanges>,
  ignore_list: bool,
}

#[napi]
impl BindingMagicString<'_> {
  #[napi(constructor)]
  pub fn new(source: String, options: Option<BindingMagicStringOptions>) -> Self {
    let utf16_to_byte_mapper = Utf16ToByteMapper::new(&source);
    let opts = options.unwrap_or_default();
    let offset = opts.offset.unwrap_or(0);
    let indent_exclusion_ranges =
      opts.indent_exclusion_ranges.map(IndentExclusionRanges::from_either);
    let ignore_list = opts.ignore_list.unwrap_or(false);
    let magic_string_options = MagicStringOptions { filename: opts.filename, ignore_list };
    Self {
      inner: MagicString::with_options(source, magic_string_options),
      utf16_to_byte_mapper,
      offset,
      indent_exclusion_ranges,
      ignore_list,
    }
  }

  #[napi(getter)]
  pub fn original(&self) -> &str {
    self.inner.source()
  }

  #[napi(getter)]
  pub fn filename(&self) -> Option<&str> {
    self.inner.filename()
  }

  #[napi(getter)]
  pub fn indent_exclusion_ranges(&self) -> Option<Either<Vec<Vec<i64>>, Vec<i64>>> {
    self.indent_exclusion_ranges.as_ref().map(IndentExclusionRanges::to_either)
  }

  #[napi(getter)]
  pub fn ignore_list(&self) -> bool {
    self.ignore_list
  }

  #[napi(getter)]
  pub fn get_offset(&self) -> i64 {
    self.offset
  }

  #[napi(setter)]
  pub fn set_offset(&mut self, offset: i64) {
    self.offset = offset;
  }

  /// Performs regex-based replace on the original source string.
  /// When `global` is true, replaces all matches; otherwise replaces only the first.
  /// Handles `$&`, `$$`, and `$N` substitution patterns in the replacement string.
  ///
  /// NOTE: Uses `HybridRegex` which tries `regex::Regex` first (orders of magnitude
  /// faster) and only falls back to `regress::Regex` when the pattern uses syntax
  /// not supported by the `regex` crate (e.g. backreferences, lookaround).
  /// Sticky (`y`) flag always uses the `regress` path since `regex` doesn't support it,
  /// and `lastIndex` is respected via `find_from`.
  fn regex_replace(&mut self, js_regex: &JsRegExp, replacement: &str) -> napi::Result<Option<u32>> {
    let global = js_regex.flags.contains('g');
    let flags_without_g: String = js_regex.flags.chars().filter(|&c| c != 'g').collect();
    let reg = HybridRegex::with_flags(&js_regex.source, &flags_without_g)
      .map_err(|e| napi::Error::from_reason(format!("Invalid regex: {e}")))?;
    let source = self.inner.source();

    // Track last match end (byte offset) for lastIndex writeback.
    // This includes no-op matches (where replacement == matched text).
    let mut last_match_end: Option<u32> = None;

    // Collect into Vec to release the borrow on `source` before mutating `self.inner`.
    #[expect(clippy::cast_possible_truncation)]
    let overwrites: Vec<(u32, u32, String)> = match &reg {
      HybridRegex::Optimize(r) => {
        // The `regex` crate path is only used for non-sticky patterns (the `y` flag
        // causes `regex::Regex::new` to fail, falling back to regress).
        // For non-sticky regexes, JS resets `lastIndex` before matching, so we
        // always start from the beginning.
        let iter = r.captures_iter(source);
        let iter = if global {
          itertools::Either::Left(iter)
        } else {
          itertools::Either::Right(iter.take(1))
        };
        iter
          .filter_map(|caps| {
            let full = caps.get(0).unwrap();
            let matched = full.as_str();
            let group_count = caps.len();
            last_match_end = Some(full.end() as u32);
            let rep = apply_replacement_regex(replacement, matched, &caps, group_count);
            (rep != matched).then(|| (full.start() as u32, full.end() as u32, rep))
          })
          .collect()
      }
      HybridRegex::Ecma(r) => {
        let is_sticky = js_regex.flags.contains('y');
        // For global regexes, JS resets lastIndex to 0 before matching.
        // For non-global sticky, use the caller's lastIndex (converted from UTF-16 to byte offset).
        // If lastIndex is out of bounds, sticky must immediately fail (no match).
        let start = if global || !is_sticky {
          0
        } else {
          match self.utf16_to_byte_mapper.utf16_to_byte(js_regex.last_index as u32) {
            Some(byte_offset) => byte_offset as usize,
            None => return Ok(None),
          }
        };

        if is_sticky {
          // Sticky: only accept contiguous matches starting at `start`.
          // For non-global sticky, this is at most one match.
          let mut results = Vec::new();
          let mut pos = start;
          for m in r.find_from(source, start) {
            if m.range.start != pos {
              break; // non-contiguous — stop
            }
            pos = m.range.end;
            last_match_end = Some(m.range.end as u32);
            let matched = &source[m.range.clone()];
            let rep = apply_replacement_regress(replacement, matched, &m, source);
            if rep != matched {
              results.push((m.range.start as u32, m.range.end as u32, rep));
            }
            if !global {
              break; // non-global: one match only
            }
          }
          results
        } else {
          // Non-sticky
          let iter = r.find_from(source, 0);
          let iter = if global {
            itertools::Either::Left(iter)
          } else {
            itertools::Either::Right(iter.take(1))
          };
          iter
            .filter_map(|m| {
              last_match_end = Some(m.range.end as u32);
              let matched = &source[m.range.clone()];
              let rep = apply_replacement_regress(replacement, matched, &m, source);
              (rep != matched).then_some((m.range.start as u32, m.range.end as u32, rep))
            })
            .collect()
        }
      }
    };

    // Convert byte offset back to UTF-16 code units for JS lastIndex writeback.
    let last_match_end_utf16 =
      last_match_end.and_then(|b| self.utf16_to_byte_mapper.byte_to_utf16(b));

    for (start, end, rep) in overwrites {
      self
        .inner
        .update_with(start, end, rep, UpdateOptions { overwrite: true, keep_original: false })
        .map_err(napi::Error::from_reason)?;
    }
    Ok(last_match_end_utf16)
  }

  /// Applies `self.offset` to a u32 character index.
  /// Returns an error if the resulting index would be negative (underflow).
  #[inline]
  fn apply_offset_u32(&self, index: u32) -> napi::Result<u32> {
    let result = i64::from(index) + self.offset;
    if result < 0 || result > i64::from(u32::MAX) {
      return Err(napi::Error::from_reason(format!(
        "index {index} is out of bounds with offset {}",
        self.offset
      )));
    }
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(result as u32)
  }

  /// Applies `self.offset` to an i64 character index.
  /// Uses saturating addition to avoid undefined behaviour on extreme offset values.
  #[inline]
  fn apply_offset_i64(&self, index: i64) -> i64 {
    index.saturating_add(self.offset)
  }

  #[napi]
  pub fn replace<'s>(
    &'s mut self,
    this: This<'s>,
    from: String,
    to: String,
  ) -> napi::Result<This<'s>> {
    self.inner.replace(&from, to).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  #[napi]
  pub fn replace_all<'s>(
    &'s mut self,
    this: This<'s>,
    from: String,
    to: String,
  ) -> napi::Result<This<'s>> {
    self.inner.replace_all(&from, to).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  /// Returns the UTF-16 offset past the last match, or -1 if no match was found.
  /// The JS wrapper uses this to update `lastIndex` on the caller's RegExp.
  /// Global/sticky behavior is derived from the regex's own flags.
  #[napi(js_name = "replaceRegex")]
  pub fn replace_regex(
    &mut self,
    #[napi(ts_arg_type = "RegExp")] from: JsRegExp,
    to: String,
  ) -> napi::Result<i32> {
    let last_end = self.regex_replace(&from, &to)?;
    #[expect(clippy::cast_possible_wrap)]
    Ok(last_end.map_or(-1, |v| v as i32))
  }

  #[napi]
  pub fn prepend<'s>(&'s mut self, this: This<'s>, content: String) -> This<'s> {
    self.inner.prepend(content);
    this
  }

  #[napi]
  pub fn append<'s>(&'s mut self, this: This<'s>, content: String) -> This<'s> {
    self.inner.append(content);
    this
  }

  #[napi]
  pub fn prepend_left<'s>(
    &'s mut self,
    this: This<'s>,
    index: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    // Match original magic-string: out-of-bound indices fall through to prepend_intro
    match self.utf16_to_byte_mapper.utf16_to_byte(self.apply_offset_u32(index)?) {
      Some(byte_index) => {
        self.inner.prepend_left(byte_index, content);
      }
      None => {
        self.inner.prepend(content);
      }
    }
    Ok(this)
  }

  #[napi]
  pub fn prepend_right<'s>(
    &'s mut self,
    this: This<'s>,
    index: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    // Match original magic-string: out-of-bound indices fall through to prepend_outro
    match self.utf16_to_byte_mapper.utf16_to_byte(self.apply_offset_u32(index)?) {
      Some(byte_index) => {
        self.inner.prepend_right(byte_index, content);
      }
      None => {
        self.inner.prepend_outro(content);
      }
    }
    Ok(this)
  }

  #[napi]
  pub fn append_left<'s>(
    &'s mut self,
    this: This<'s>,
    index: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    // Match original magic-string: out-of-bound indices fall through to append_intro
    match self.utf16_to_byte_mapper.utf16_to_byte(self.apply_offset_u32(index)?) {
      Some(byte_index) => {
        self.inner.append_left(byte_index, content);
      }
      None => {
        self.inner.append_intro(content);
      }
    }
    Ok(this)
  }

  #[napi]
  pub fn append_right<'s>(
    &'s mut self,
    this: This<'s>,
    index: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    // Match original magic-string: out-of-bound indices fall through to append_outro
    match self.utf16_to_byte_mapper.utf16_to_byte(self.apply_offset_u32(index)?) {
      Some(byte_index) => {
        self.inner.append_right(byte_index, content);
      }
      None => {
        self.inner.append(content);
      }
    }
    Ok(this)
  }

  #[napi]
  pub fn overwrite<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(start)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid start character index"))?;
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(end)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid end character index"))?;
    self
      .inner
      .update_with(
        start_byte,
        end_byte,
        content,
        string_wizard::UpdateOptions { overwrite: true, keep_original: false },
      )
      .map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  #[napi]
  pub fn to_string(&self) -> String {
    self.inner.to_string()
  }

  #[napi]
  pub fn has_changed(&self) -> bool {
    self.inner.has_changed()
  }

  #[napi]
  pub fn length(&self) -> u32 {
    // MagicString::len() returns usize (length of generated output)
    #[expect(clippy::cast_possible_truncation, reason = "files are < 4GB")]
    {
      self.inner.len() as u32
    }
  }

  #[napi]
  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  #[napi]
  pub fn remove<'s>(&'s mut self, this: This<'s>, start: u32, end: u32) -> napi::Result<This<'s>> {
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(start)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid start character index"))?;
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(end)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid end character index"))?;
    self.inner.remove(start_byte, end_byte).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  #[napi]
  pub fn update<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(start)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid start character index"))?;
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(end)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid end character index"))?;
    self.inner.update(start_byte, end_byte, content).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  #[napi]
  pub fn relocate<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    to: u32,
  ) -> napi::Result<This<'s>> {
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(start)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid start character index"))?;
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(end)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid end character index"))?;
    let to_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(to)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid to character index"))?;
    self.inner.relocate(start_byte, end_byte, to_byte).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  /// Alias for `relocate` to match the original magic-string API.
  /// Moves the characters from `start` to `end` to `index`.
  /// Returns `this` for method chaining.
  #[napi(js_name = "move")]
  pub fn move_<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    index: u32,
  ) -> napi::Result<This<'s>> {
    self.relocate(this, start, end, index)
  }

  #[napi]
  pub fn indent<'s>(
    &'s mut self,
    this: This<'s>,
    indentor: Option<String>,
    options: Option<BindingIndentOptions>,
  ) -> This<'s> {
    // Per-call exclude takes priority; fall back to constructor's indentExclusionRanges.
    let explicit_exclude = options.and_then(|opts| opts.exclude);
    let exclude_ranges = if let Some(ref e) = explicit_exclude {
      normalize_exclude_ranges(e, &self.utf16_to_byte_mapper, self.offset)
    } else if let Some(ref stored) = self.indent_exclusion_ranges {
      normalize_exclude_ranges(&stored.to_either(), &self.utf16_to_byte_mapper, self.offset)
    } else {
      vec![]
    };

    self.inner.indent_with(string_wizard::IndentOptions {
      indentor: indentor.as_deref(),
      exclude: &exclude_ranges,
    });
    this
  }

  /// Trims whitespace or specified characters from the start and end.
  #[napi]
  pub fn trim<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim(char_type.as_deref());
    this
  }

  /// Trims whitespace or specified characters from the start.
  #[napi]
  pub fn trim_start<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim_start(char_type.as_deref());
    this
  }

  /// Trims whitespace or specified characters from the end.
  #[napi]
  pub fn trim_end<'s>(&'s mut self, this: This<'s>, char_type: Option<String>) -> This<'s> {
    self.inner.trim_end(char_type.as_deref());
    this
  }

  /// Trims newlines from the start and end.
  #[napi]
  pub fn trim_lines<'s>(&'s mut self, this: This<'s>) -> This<'s> {
    self.inner.trim_lines();
    this
  }

  /// Deprecated method that throws an error directing users to use prependRight or appendLeft.
  /// This matches the original magic-string API which deprecated this method.
  #[napi]
  pub fn insert(&self, _index: u32, _content: String) -> napi::Result<()> {
    Err(napi::Error::from_reason(
      "magicString.insert(...) is deprecated. Use prependRight(...) or appendLeft(...)",
    ))
  }

  /// Returns a clone of the MagicString instance.
  #[napi(js_name = "clone")]
  #[must_use]
  pub fn clone_instance(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      utf16_to_byte_mapper: self.utf16_to_byte_mapper.clone(),
      offset: self.offset,
      indent_exclusion_ranges: self.indent_exclusion_ranges.clone(),
      ignore_list: self.ignore_list,
    }
  }

  /// Returns the last character of the generated string, or an empty string if empty.
  #[napi]
  pub fn last_char(&self) -> String {
    self.inner.last_char().map(|c| c.to_string()).unwrap_or_default()
  }

  /// Returns the content after the last newline in the generated string.
  #[napi]
  pub fn last_line(&self) -> String {
    self.inner.last_line()
  }

  /// Returns the guessed indentation string, or `\t` if none is found.
  #[napi]
  pub fn get_indent_string(&self) -> &str {
    self.inner.get_indent_string()
  }

  /// Returns a clone with content outside the specified range removed.
  #[napi]
  pub fn snip(&self, start: u32, end: u32) -> napi::Result<Self> {
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(start)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid start character index"))?;
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(self.apply_offset_u32(end)?)
      .ok_or_else(|| napi::Error::from_reason("Invalid end character index"))?;
    Ok(Self {
      inner: self.inner.snip(start_byte, end_byte).map_err(napi::Error::from_reason)?,
      utf16_to_byte_mapper: self.utf16_to_byte_mapper.clone(),
      offset: self.offset,
      indent_exclusion_ranges: self.indent_exclusion_ranges.clone(),
      ignore_list: self.ignore_list,
    })
  }

  /// Resets the portion of the string from `start` to `end` to its original content.
  /// This undoes any modifications made to that range.
  /// Supports negative indices (counting from the end).
  #[napi]
  pub fn reset<'s>(&'s mut self, this: This<'s>, start: i64, end: i64) -> napi::Result<This<'s>> {
    // Apply offset, then handle negative indices (matching original magic-string behavior)
    let start = self.utf16_to_byte_mapper.normalize_index(self.apply_offset_i64(start));
    let end = self.utf16_to_byte_mapper.normalize_index(self.apply_offset_i64(end));

    // Convert character indices to byte indices
    // indices are non-negative after normalize_index and files are < 4GB
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let start_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(start as u32)
      .ok_or_else(|| napi::Error::from_reason("Character is out of bounds"))?;

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let end_byte = self
      .utf16_to_byte_mapper
      .utf16_to_byte(end as u32)
      .ok_or_else(|| napi::Error::from_reason("Character is out of bounds"))?;

    self.inner.reset(start_byte, end_byte).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  /// Returns the content between the specified UTF-16 code unit positions (JS string indices).
  /// Supports negative indices (counting from the end).
  ///
  /// When an index falls in the middle of a surrogate pair, the lone surrogate is
  /// included in the result (matching the original magic-string / JS behavior).
  /// This is done by returning a UTF-16 encoded JS string via `napi_create_string_utf16`.
  #[napi]
  pub fn slice<'env>(
    &self,
    env: &'env Env,
    start: Option<i64>,
    end: Option<i64>,
  ) -> napi::Result<JsString<'env>> {
    // Apply offset to both start and end (including defaults), then normalize negatives
    let start = self.apply_offset_i64(start.unwrap_or(0));

    // When end is omitted, default to the internal string end (char_count) directly,
    // without shifting by offset. Applying offset to the default would shift the end
    // left for negative offsets, collapsing the range to empty.
    let end = match end {
      Some(e) => self.apply_offset_i64(e),
      None => self.utf16_to_byte_mapper.utf16_len(),
    };

    // Handle negative indices (matching original magic-string behavior)
    let start = self.utf16_to_byte_mapper.normalize_index(start);
    let end = self.utf16_to_byte_mapper.normalize_index(end);

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let start_u32 = start as u32;
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let end_u32 = end as u32;

    // Fetch the mapping entries once. If start/end fall on a low surrogate (middle
    // of a surrogate pair), we need special handling:
    // - start at LOW: prepend the lone low surrogate, UTF-8 slice starts after the char.
    // - end at LOW: use the previous entry's byte offset (before the char) and append
    //   the lone high surrogate.
    // - HIGH surrogate positions already have the correct byte offset (before the char).
    let total_len = self.utf16_to_byte_mapper.total_len();
    let start_entry = self.utf16_to_byte_mapper.get(start_u32);
    let end_entry = self.utf16_to_byte_mapper.get(end_u32);

    // When start == end, the result is always empty regardless of surrogate position.
    // Only check surrogates when the range is non-empty.
    let (start_is_low, end_is_low, end_prev_entry) = if start_u32 == end_u32 {
      (false, false, None)
    } else {
      let start_is_low = start_entry.is_some_and(Utf16Mapping::is_low_surrogate);
      let end_is_low = end_entry.is_some_and(Utf16Mapping::is_low_surrogate);
      // For forward ranges (start < end), when end is a low surrogate we adjust the
      // byte offset to exclude the character entirely and later append the high surrogate.
      // For reversed/moved ranges (start > end) this byte-offset trick does not work
      // because the inner slice sees the end chunk before the start chunk, so we
      // post-process instead (see below).
      let end_prev = if end_is_low && start_u32 < end_u32 {
        debug_assert!(end_u32 > 0, "low surrogate cannot appear at index 0");
        self.utf16_to_byte_mapper.get(end_u32 - 1)
      } else {
        None
      };
      (start_is_low, end_is_low, end_prev)
    };

    let start_byte = start_entry.map_or(total_len, |e| e.byte_offset);
    let end_byte = if let Some(prev) = end_prev_entry {
      // End falls on a low surrogate (forward range) — use the high surrogate's byte_offset
      // (before the character) so the UTF-8 slice excludes it.
      prev.byte_offset
    } else {
      end_entry.map_or(total_len, |e| e.byte_offset)
    };
    let utf8_result =
      self.inner.slice(start_byte, Some(end_byte)).map_err(napi::Error::from_reason)?;

    // Fast path: no lone surrogates involved — return the UTF-8 string directly,
    // avoiding the UTF-16 transcoding and allocation.
    if !start_is_low && !end_is_low {
      return env.create_string(&utf8_result);
    }

    // Slow path: build UTF-16 buffer with lone surrogates at the boundaries.
    let mut utf16_buf: Vec<u16> = Vec::new();

    // Only prepend the start's low surrogate for forward ranges. For reversed ranges
    // without moves the inner slice returns "" and we should return "" unchanged.
    if start_u32 < end_u32 {
      if let Some(entry) = start_entry.filter(|e| e.is_low_surrogate()) {
        utf16_buf.push(entry.surrogate);
      }
    }

    utf16_buf.extend(utf8_result.encode_utf16());

    if let Some(high_entry) = end_prev_entry {
      // Forward range: emoji was excluded by byte-offset adjustment, append the high surrogate.
      utf16_buf.push(high_entry.surrogate);
    } else if end_is_low && !utf8_result.is_empty() {
      // Reversed/moved range: the inner slice included the full emoji character.
      // Remove the trailing low surrogate to leave only the high surrogate,
      // matching JS String.prototype.slice behavior at surrogate boundaries.
      if let Some(&last) = utf16_buf.last() {
        if (0xDC00..=0xDFFF).contains(&last) {
          utf16_buf.pop();
        }
      }
    }

    env.create_string_utf16(&utf16_buf)
  }

  /// Generates a source map for the transformations applied to this MagicString.
  /// Returns a BindingSourceMap object with version, file, sources, sourcesContent, names, mappings.
  #[napi]
  pub fn generate_map(&self, options: Option<BindingSourceMapOptions>) -> BindingSourceMap {
    let opts = options.unwrap_or_default();
    let hires = match &opts.hires {
      Some(Either::A(true)) => string_wizard::Hires::True,
      Some(Either::B(s)) if s == "boundary" => string_wizard::Hires::Boundary,
      _ => string_wizard::Hires::False,
    };
    let source_map = self.inner.source_map(SourceMapOptions {
      source: opts.source.map(Into::into).unwrap_or_else(|| "".into()),
      include_content: opts.include_content.unwrap_or(false),
      hires,
    });

    // If file option is provided, reconstruct the source map with the file field
    let source_map = if let Some(file) = opts.file {
      let mut m = SourceMap::new(
        Some(Arc::from(file)),
        source_map.get_names().map(Arc::clone).collect(),
        None,
        source_map.get_sources().map(Arc::clone).collect(),
        source_map.get_source_contents().map(|x| x.map(Arc::clone)).collect(),
        source_map.get_tokens().collect::<Vec<_>>().into_boxed_slice(),
        None,
      );
      if self.ignore_list {
        m.set_x_google_ignore_list(vec![0]);
      }
      m
    } else {
      source_map
    };

    BindingSourceMap { json: source_map.to_json() }
  }

  /// Generates a decoded source map for the transformations applied to this MagicString.
  /// Returns a BindingDecodedMap object with mappings as an array of arrays.
  #[napi]
  pub fn generate_decoded_map(
    &self,
    options: Option<BindingSourceMapOptions>,
  ) -> BindingDecodedMap {
    let opts = options.unwrap_or_default();
    let hires = match &opts.hires {
      Some(Either::A(true)) => string_wizard::Hires::True,
      Some(Either::B(s)) if s == "boundary" => string_wizard::Hires::Boundary,
      _ => string_wizard::Hires::False,
    };
    let source_map = self.inner.source_map(SourceMapOptions {
      source: opts.source.map(Into::into).unwrap_or_else(|| "".into()),
      include_content: opts.include_content.unwrap_or(false),
      hires,
    });

    // If file option is provided, reconstruct the source map with the file field
    let source_map = if let Some(file) = opts.file {
      let mut m = SourceMap::new(
        Some(Arc::from(file)),
        source_map.get_names().map(Arc::clone).collect(),
        None,
        source_map.get_sources().map(Arc::clone).collect(),
        source_map.get_source_contents().map(|x| x.map(Arc::clone)).collect(),
        source_map.get_tokens().collect::<Vec<_>>().into_boxed_slice(),
        None,
      );
      if self.ignore_list {
        m.set_x_google_ignore_list(vec![0]);
      }
      m
    } else {
      source_map
    };

    let json = source_map.to_json();
    BindingDecodedMap { inner: source_map, json }
  }
}

/// Applies `$&`, `$$`, and `$N` substitution patterns in a replacement string,
/// matching the original magic-string `_replaceRegexp` semantics (which differ slightly
/// from strict ECMAScript spec — e.g. `$0` resolves to the full match, all consecutive
/// digits are parsed as one group number, not just 1-2 digits).
///
/// `group_count` is the total number of groups including the full match (i.e. JS `match.length`).
/// `get_group` returns the matched text for group index `n` (0 = full match), or `None`.
fn apply_replacement<'a>(
  replacement: &str,
  matched: &str,
  group_count: usize,
  get_group: impl Fn(usize) -> Option<&'a str>,
) -> String {
  // Fast path: no substitution tokens — return replacement as-is.
  if !replacement.contains('$') {
    return replacement.to_owned();
  }
  let mut result = String::with_capacity(replacement.len());
  let bytes = replacement.as_bytes();
  let len = bytes.len();
  let mut i = 0;
  while i < len {
    if bytes[i] == b'$' && i + 1 < len {
      match bytes[i + 1] {
        b'$' => {
          result.push('$');
          i += 2;
        }
        b'&' => {
          result.push_str(matched);
          i += 2;
        }
        b'0'..=b'9' => {
          // Parse all consecutive digits after $
          let start = i + 1;
          let mut end = start + 1;
          while end < len && bytes[end].is_ascii_digit() {
            end += 1;
          }
          let num: usize = replacement[start..end].parse().unwrap_or(0);
          // Match JS semantics: check if group exists (num < match.length)
          if num < group_count {
            if let Some(text) = get_group(num) {
              result.push_str(text);
            }
            // group matched empty or didn't participate — output nothing
            i = end;
          } else {
            // No such group — keep literal `$N`
            result.push_str(&replacement[i..end]);
            i = end;
          }
        }
        _ => {
          result.push('$');
          i += 1;
        }
      }
    } else {
      // Find the next '$' or end of string; copy the span in one shot.
      // This is correct for multi-byte UTF-8 since we only split on ASCII '$'.
      let span_start = i;
      i += 1;
      while i < len && bytes[i] != b'$' {
        i += 1;
      }
      result.push_str(&replacement[span_start..i]);
    }
  }
  result
}

/// `apply_replacement` adapter for `regex::Captures` (fast path).
fn apply_replacement_regex(
  replacement: &str,
  matched: &str,
  caps: &regex::Captures<'_>,
  group_count: usize,
) -> String {
  apply_replacement(replacement, matched, group_count, |n| caps.get(n).map(|m| m.as_str()))
}

/// `apply_replacement` adapter for `regress::Match` (slow/fallback path).
fn apply_replacement_regress(
  replacement: &str,
  matched: &str,
  m: &regress::Match,
  source: &str,
) -> String {
  let group_count = 1 + m.captures.len();
  apply_replacement(replacement, matched, group_count, |n| m.group(n).map(|range| &source[range]))
}
