//! Import attributes as part of module identity, aligned with Rollup 5.
//!
//! See internal-docs/import-attributes/implementation.md

use std::{cmp::Ordering, fmt::Write as _, sync::Arc};

use itertools::Itertools as _;
use oxc_str::CompactStr;
use rolldown_utils::{
  concat_string,
  ecmascript::{to_ecma_string_literal, to_module_import_export_name_with_ascii},
};

use crate::ModuleType;

const ATTRIBUTES_MARKER: &str = "attributes=";

/// The import attributes a module is requested with, e.g. `{ type: 'json' }` for
/// `import data from './data.json' with { type: 'json' }`. A JS `Record<string, string>`;
/// empty means no attributes and allocates nothing.
///
/// Entries are kept in the order JS enumerates the keys of the object that Rollup's
/// `generateIdByRawIdAndAttributes` serializes, so [`ImportAttributes::compose_module_id`]
/// is byte-identical to it: array-index keys first in ascending numeric order, then the other
/// keys sorted by UTF-16 code units. Key order of the source therefore does not matter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ImportAttributes(Option<Arc<[(CompactStr, CompactStr)]>>);

impl ImportAttributes {
  /// Later entries win over earlier ones with the same key, like assigning to a JS object.
  pub fn from_entries<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> Self
  where
    K: Into<CompactStr>,
    V: Into<CompactStr>,
  {
    let mut entries: Vec<(CompactStr, CompactStr)> =
      entries.into_iter().map(|(key, value)| (key.into(), value.into())).collect();
    if entries.is_empty() {
      return Self::default();
    }
    // A stable sort keeps duplicated keys in source order, so the last one is the one to keep.
    entries.sort_by(|(a, _), (b, _)| compare_keys(a, b));
    entries.reverse();
    entries.dedup_by(|(a, _), (b, _)| a == b);
    entries.reverse();
    Self(Some(entries.into()))
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_none()
  }

  pub fn len(&self) -> usize {
    self.0.as_ref().map_or(0, |entries| entries.len())
  }

  pub fn get(&self, key: &str) -> Option<&str> {
    self.iter().find_map(|(k, v)| (k == key).then_some(v))
  }

  /// The built-in module type the `type` attribute selects: `json`, `text` or `bytes`. It beats
  /// the file extension and `moduleTypes`, but not the `moduleType` of a load hook.
  pub fn module_type(&self) -> Option<ModuleType> {
    match self.get("type")? {
      "json" => Some(ModuleType::Json),
      "text" => Some(ModuleType::Text),
      "bytes" => Some(ModuleType::Binary),
      _ => None,
    }
  }

  pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
    self.0.iter().flat_map(|entries| entries.iter().map(|(k, v)| (k.as_str(), v.as_str())))
  }

  /// `with { type: "json" }`, or `None` without attributes.
  pub fn to_with_clause(&self, ascii_only: bool) -> Option<String> {
    if self.is_empty() {
      return None;
    }
    let entries = self
      .iter()
      .map(|(key, value)| {
        concat_string!(
          to_module_import_export_name_with_ascii(key, ascii_only),
          ": ",
          to_ecma_string_literal(value, ascii_only)
        )
      })
      .join(", ");
    Some(concat_string!("with { ", entries, " }"))
  }

  /// `rawId?attributes=<encodeURIComponent(JSON.stringify(attributes))>`, or `raw_id` itself
  /// when there are no attributes. Mirrors Rollup's `generateIdByRawIdAndAttributes`.
  pub fn compose_module_id(&self, raw_id: &str) -> String {
    let mut id = String::with_capacity(raw_id.len() + 1 + ATTRIBUTES_MARKER.len() + 32);
    id.push_str(raw_id);
    if self.is_empty() {
      return id;
    }
    id.push(if raw_id.contains('?') { '&' } else { '?' });
    id.push_str(ATTRIBUTES_MARKER);
    let mut json = String::from("{");
    for (i, (key, value)) in self.iter().enumerate() {
      if i > 0 {
        json.push(',');
      }
      push_json_string(&mut json, key);
      json.push(':');
      push_json_string(&mut json, value);
    }
    json.push('}');
    push_uri_component(&mut id, &json);
    id
  }

  /// Splits a module id into its raw id and attributes. Mirrors Rollup's `getRawIdAndAttributes`:
  /// the suffix after the last `?attributes=` / `&attributes=` is only accepted when composing
  /// it again yields exactly `id`, so ids that merely look similar stay opaque.
  ///
  /// Returns `None` when `id` carries no attributes.
  pub fn parse_module_id(id: &str) -> Option<(&str, Self)> {
    let marker_start = [id.rfind("?attributes="), id.rfind("&attributes=")].into_iter().max()??;
    let raw_id = &id[..marker_start];
    let decoded = decode_uri_component(&id[marker_start + 1 + ATTRIBUTES_MARKER.len()..])?;
    let serde_json::Value::Object(object) = serde_json::from_str(&decoded).ok()? else {
      return None;
    };
    let entries = object
      .iter()
      .map(|(key, value)| value.as_str().map(|value| (key.as_str(), value)))
      .collect::<Option<Vec<_>>>()?;
    let attributes = Self::from_entries(entries);
    (!attributes.is_empty() && attributes.compose_module_id(raw_id) == id)
      .then_some((raw_id, attributes))
  }
}

/// The order of own string keys of a JS object whose properties were created in sorted order.
fn compare_keys(a: &str, b: &str) -> Ordering {
  match (as_array_index(a), as_array_index(b)) {
    (Some(a), Some(b)) => a.cmp(&b),
    (Some(_), None) => Ordering::Less,
    (None, Some(_)) => Ordering::Greater,
    // JS compares strings by UTF-16 code units, which differs from Rust's byte order for
    // characters outside the BMP.
    (None, None) => a.encode_utf16().cmp(b.encode_utf16()),
  }
}

/// An array index is the canonical decimal form of an integer in `0..=2^32 - 2`.
fn as_array_index(key: &str) -> Option<u32> {
  let bytes = key.as_bytes();
  if bytes.is_empty() || bytes.len() > 10 || !bytes.iter().all(u8::is_ascii_digit) {
    return None;
  }
  if bytes.len() > 1 && bytes[0] == b'0' {
    return None;
  }
  let value: u64 = key.parse().ok()?;
  u32::try_from(value).ok().filter(|value| *value != u32::MAX)
}

/// `JSON.stringify` for a string.
fn push_json_string(out: &mut String, value: &str) {
  out.push('"');
  for c in value.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\u{08}' => out.push_str("\\b"),
      '\u{0C}' => out.push_str("\\f"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      c if c < ' ' => {
        let _ = write!(out, "\\u{:04x}", c as u32);
      }
      c => out.push(c),
    }
  }
  out.push('"');
}

/// `encodeURIComponent`.
fn push_uri_component(out: &mut String, value: &str) {
  for byte in value.bytes() {
    if byte.is_ascii_alphanumeric()
      || matches!(byte, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')')
    {
      out.push(byte as char);
    } else {
      let _ = write!(out, "%{byte:02X}");
    }
  }
}

/// `decodeURIComponent`, returning `None` where it would throw.
fn decode_uri_component(value: &str) -> Option<String> {
  let bytes = value.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'%' {
      let hex = bytes.get(i + 1..i + 3)?;
      decoded.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
      i += 3;
    } else {
      decoded.push(bytes[i]);
      i += 1;
    }
  }
  String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
  use super::ImportAttributes;

  fn attrs(entries: &[(&str, &str)]) -> ImportAttributes {
    ImportAttributes::from_entries(entries.iter().copied())
  }

  #[test]
  fn compose_matches_rollup() {
    // Expected values come from Rollup 5's `generateIdByRawIdAndAttributes`.
    assert_eq!(attrs(&[]).compose_module_id("/src/a.js"), "/src/a.js");
    assert_eq!(
      attrs(&[("type", "json")]).compose_module_id("/src/config.json"),
      "/src/config.json?attributes=%7B%22type%22%3A%22json%22%7D"
    );
    assert_eq!(
      attrs(&[("type", "image"), ("size", "200")]).compose_module_id("/src/logo.png"),
      "/src/logo.png?attributes=%7B%22size%22%3A%22200%22%2C%22type%22%3A%22image%22%7D"
    );
    assert_eq!(
      attrs(&[("type", "text")]).compose_module_id("/src/icon.svg?raw"),
      "/src/icon.svg?raw&attributes=%7B%22type%22%3A%22text%22%7D"
    );
  }

  #[test]
  fn compose_orders_keys_like_js() {
    // Array-index keys come first in numeric order, then the rest by UTF-16 code units
    // (`'\u{ff61}' > '😀'` in JS although its UTF-8 bytes sort first).
    let attributes = attrs(&[("b", "1"), ("10", "2"), ("2", "3"), ("😀", "4"), ("\u{ff61}", "5")]);
    let keys: Vec<_> = attributes.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["2", "10", "b", "😀", "\u{ff61}"]);
    // `01` and `4294967295` are not array indices.
    let attributes = attrs(&[("01", ""), ("4294967295", ""), ("4294967294", ""), ("0", "")]);
    let keys: Vec<_> = attributes.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["0", "4294967294", "01", "4294967295"]);
  }

  #[test]
  fn compose_escapes_like_js() {
    // JSON.stringify('"\\\n\u0001 é') === '"\\"\\\\\\n\\u0001 é"'
    assert_eq!(
      attrs(&[("k", "\"\\\n\u{1}\u{2028}é !'()*~")]).compose_module_id("a"),
      "a?attributes=%7B%22k%22%3A%22%5C%22%5C%5C%5Cn%5Cu0001%E2%80%A8%C3%A9%20!'()*~%22%7D"
    );
  }

  #[test]
  fn last_duplicated_key_wins() {
    assert_eq!(attrs(&[("type", "a"), ("x", "1"), ("type", "b")]).get("type"), Some("b"));
  }

  #[test]
  fn parse_round_trips() {
    for (raw_id, attributes) in [
      ("/src/a.json", attrs(&[("type", "json")])),
      ("/src/icon.svg?raw", attrs(&[("type", "text")])),
      ("\0virtual", attrs(&[("size", "200"), ("type", "image")])),
      ("a", attrs(&[("k", "\"\\\n\u{1}é😀")])),
    ] {
      let id = attributes.compose_module_id(raw_id);
      assert_eq!(ImportAttributes::parse_module_id(&id), Some((raw_id, attributes)));
    }
  }

  #[test]
  fn parse_keeps_opaque_ids() {
    for id in [
      "/src/a.js",
      "/src/a.js?raw",
      "virtual?attributes=foo",
      // Valid JSON, but not what composing would produce.
      "a?attributes=%7B%7D",
      "a?attributes=%7B%22type%22%3A1%7D",
      "a?attributes=%7B%20%22type%22%3A%22json%22%7D",
      "a?attributes=%7b%22type%22%3a%22json%22%7d",
      "a?attributes=%E0%A4%A",
    ] {
      assert_eq!(ImportAttributes::parse_module_id(id), None, "{id}");
    }
  }
}
