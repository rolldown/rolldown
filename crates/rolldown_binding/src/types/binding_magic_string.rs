#![expect(clippy::inherent_to_string)]
use std::sync::Arc;

use napi::bindgen_prelude::{Either, This};
use napi_derive::napi;
use rolldown_sourcemap::{JSONSourceMap, SourceMap};
use rolldown_utils::base64::to_standard_base64;
use serde::Serialize;
use string_wizard::{MagicString, MagicStringOptions, SourceMapOptions};

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
}

#[derive(Clone)]
struct CharToByteMapper {
  char_to_byte: Vec<u32>,
}

impl CharToByteMapper {
  #[expect(clippy::cast_possible_truncation)]
  fn new(s: &str) -> Self {
    let mut char_to_byte = Vec::with_capacity(s.chars().count() + 1);
    char_to_byte.push(0); // char 0 is at byte 0

    let mut byte_offset = 0u32;
    for ch in s.chars() {
      byte_offset += ch.len_utf16() as u32;
      char_to_byte.push(byte_offset);
    }

    Self { char_to_byte }
  }

  #[inline]
  fn char_to_byte(&self, char_offset: u32) -> Option<u32> {
    self.char_to_byte.get(char_offset as usize).copied()
  }

  /// Returns the character count (number of characters in the string).
  fn char_count(&self) -> i64 {
    // The vector has N+1 elements for N characters (stores byte offset after each char)
    #[expect(clippy::cast_possible_wrap)]
    let count = (self.char_to_byte.len() - 1) as i64;
    count
  }

  /// Normalizes a potentially negative index to a positive index.
  /// Negative indices count from the end of the string (matching original magic-string behavior).
  fn normalize_index(&self, index: i64) -> i64 {
    let char_count = self.char_count();
    if char_count > 0 && index < 0 {
      ((index % char_count) + char_count) % char_count
    } else {
      index
    }
  }
}

#[napi(object)]
#[derive(Default)]
pub struct BindingMagicStringOptions {
  pub filename: Option<String>,
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
}

#[napi]
pub struct BindingMagicString<'a> {
  pub(crate) inner: MagicString<'a>,
  char_to_byte_mapper: CharToByteMapper,
}

#[napi]
impl BindingMagicString<'_> {
  #[napi(constructor)]
  pub fn new(source: String, options: Option<BindingMagicStringOptions>) -> Self {
    let char_to_byte_mapper = CharToByteMapper::new(&source);
    let opts = options.unwrap_or_default();
    let magic_string_options = MagicStringOptions { filename: opts.filename };
    Self { inner: MagicString::with_options(source, magic_string_options), char_to_byte_mapper }
  }

  #[napi(getter)]
  pub fn filename(&self) -> Option<String> {
    self.inner.filename().map(String::from)
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
  pub fn prepend_left<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index = self.char_to_byte_mapper.char_to_byte(index).expect("Invalid character index");
    self.inner.prepend_left(byte_index, content);
    this
  }

  #[napi]
  pub fn prepend_right<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index = self.char_to_byte_mapper.char_to_byte(index).expect("Invalid character index");
    self.inner.prepend_right(byte_index, content);
    this
  }

  #[napi]
  pub fn append_left<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index = self.char_to_byte_mapper.char_to_byte(index).expect("Invalid character index");
    self.inner.append_left(byte_index, content);
    this
  }

  #[napi]
  pub fn append_right<'s>(&'s mut self, this: This<'s>, index: u32, content: String) -> This<'s> {
    let byte_index = self.char_to_byte_mapper.char_to_byte(index).expect("Invalid character index");
    self.inner.append_right(byte_index, content);
    this
  }

  #[napi]
  pub fn overwrite<'s>(
    &'s mut self,
    this: This<'s>,
    start: u32,
    end: u32,
    content: String,
  ) -> napi::Result<This<'s>> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start).expect("Invalid start character index");
    let end_byte = self.char_to_byte_mapper.char_to_byte(end).expect("Invalid end character index");
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
  // TODO: should use `&str` instead. (claude code) Attempt failed due to generates new String from MagicString internal representation
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
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start).expect("Invalid start character index");
    let end_byte = self.char_to_byte_mapper.char_to_byte(end).expect("Invalid end character index");
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
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start).expect("Invalid start character index");
    let end_byte = self.char_to_byte_mapper.char_to_byte(end).expect("Invalid end character index");
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
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start).expect("Invalid start character index");
    let end_byte = self.char_to_byte_mapper.char_to_byte(end).expect("Invalid end character index");
    let to_byte = self.char_to_byte_mapper.char_to_byte(to).expect("Invalid to character index");
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
  pub fn indent<'s>(&'s mut self, this: This<'s>, indentor: Option<String>) -> This<'s> {
    if let Some(indentor) = indentor {
      self
        .inner
        .indent_with(string_wizard::IndentOptions { indentor: Some(&indentor), exclude: &[] });
    } else {
      self.inner.indent();
    }
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
    Self { inner: self.inner.clone(), char_to_byte_mapper: self.char_to_byte_mapper.clone() }
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

  /// Returns a clone with content outside the specified range removed.
  #[napi]
  pub fn snip(&self, start: u32, end: u32) -> napi::Result<Self> {
    let start_byte =
      self.char_to_byte_mapper.char_to_byte(start).expect("Invalid start character index");
    let end_byte = self.char_to_byte_mapper.char_to_byte(end).expect("Invalid end character index");
    Ok(Self {
      inner: self.inner.snip(start_byte, end_byte).map_err(napi::Error::from_reason)?,
      char_to_byte_mapper: self.char_to_byte_mapper.clone(),
    })
  }

  /// Resets the portion of the string from `start` to `end` to its original content.
  /// This undoes any modifications made to that range.
  /// Supports negative indices (counting from the end).
  #[napi]
  pub fn reset<'s>(&'s mut self, this: This<'s>, start: i64, end: i64) -> napi::Result<This<'s>> {
    // Handle negative indices (matching original magic-string behavior)
    let start = self.char_to_byte_mapper.normalize_index(start);
    let end = self.char_to_byte_mapper.normalize_index(end);

    // Convert character indices to byte indices
    // indices are non-negative after normalize_index and files are < 4GB
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let start_byte = self
      .char_to_byte_mapper
      .char_to_byte(start as u32)
      .ok_or_else(|| napi::Error::from_reason("Character is out of bounds"))?;

    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let end_byte = self
      .char_to_byte_mapper
      .char_to_byte(end as u32)
      .ok_or_else(|| napi::Error::from_reason("Character is out of bounds"))?;

    self.inner.reset(start_byte, end_byte).map_err(napi::Error::from_reason)?;
    Ok(this)
  }

  /// Returns the content between the specified original character positions.
  /// Supports negative indices (counting from the end).
  #[napi]
  pub fn slice(&self, start: Option<i64>, end: Option<i64>) -> napi::Result<String> {
    let start = start.unwrap_or(0);

    // Default end to char_count (original string length in characters)
    let end = end.unwrap_or(self.char_to_byte_mapper.char_count());

    // Handle negative indices (matching original magic-string behavior)
    let start = self.char_to_byte_mapper.normalize_index(start);
    let end = self.char_to_byte_mapper.normalize_index(end);

    // Convert character indices to byte indices
    // indices are non-negative after normalize_index and files are < 4GB
    #[expect(clippy::cast_possible_truncation)]
    let source_len = self.inner.source().len() as u32;
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let start_byte = self.char_to_byte_mapper.char_to_byte(start as u32).unwrap_or(source_len);
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let end_byte = self.char_to_byte_mapper.char_to_byte(end as u32).unwrap_or(source_len);

    self.inner.slice(start_byte, Some(end_byte)).map_err(napi::Error::from_reason)
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
      SourceMap::new(
        Some(Arc::from(file)),
        source_map.get_names().map(Arc::clone).collect(),
        None,
        source_map.get_sources().map(Arc::clone).collect(),
        source_map.get_source_contents().map(|x| x.map(Arc::clone)).collect(),
        source_map.get_tokens().collect::<Vec<_>>().into_boxed_slice(),
        None,
      )
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
      SourceMap::new(
        Some(Arc::from(file)),
        source_map.get_names().map(Arc::clone).collect(),
        None,
        source_map.get_sources().map(Arc::clone).collect(),
        source_map.get_source_contents().map(|x| x.map(Arc::clone)).collect(),
        source_map.get_tokens().collect::<Vec<_>>().into_boxed_slice(),
        None,
      )
    } else {
      source_map
    };

    let json = source_map.to_json();
    BindingDecodedMap { inner: source_map, json }
  }
}
