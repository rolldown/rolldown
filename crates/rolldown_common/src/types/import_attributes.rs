use std::{cmp::Ordering, sync::Arc};

use itertools::Itertools as _;
use oxc::ast::ast;
use oxc_str::CompactStr;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rolldown_utils::{
  concat_string,
  ecmascript::{to_ecma_string_literal, to_module_import_export_name_with_ascii, to_usv_str},
};
use rustc_hash::FxHashMap;

use crate::ModuleType;

const ATTRIBUTES_MARKER: &str = "attributes=";

/// The bytes `encodeURIComponent` leaves as they are.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
  .remove(b'-')
  .remove(b'_')
  .remove(b'.')
  .remove(b'!')
  .remove(b'~')
  .remove(b'*')
  .remove(b'\'')
  .remove(b'(')
  .remove(b')');

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ImportAttributes(Option<Arc<[(CompactStr, CompactStr)]>>);

impl ImportAttributes {
  pub fn from_entries<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> Self
  where
    K: Into<CompactStr>,
    V: Into<CompactStr>,
  {
    let mut sorted: Vec<(CompactStr, CompactStr)> = Vec::new();
    for (key, value) in entries {
      let (key, value) = (key.into(), value.into());
      match sorted.binary_search_by(|(existing, _)| compare_keys(existing, &key)) {
        Ok(i) => sorted[i].1 = value,
        Err(i) => sorted.insert(i, (key, value)),
      }
    }
    Self((!sorted.is_empty()).then(|| sorted.into()))
  }

  pub fn from_with_clause(with_clause: &ast::WithClause<'_>) -> Self {
    Self::from_entries(with_clause.with_entries.iter().map(|entry| {
      let key: CompactStr = match &entry.key {
        ast::ImportAttributeKey::Identifier(identifier) => identifier.name.into(),
        ast::ImportAttributeKey::StringLiteral(literal) => to_usv_str(literal.value).into(),
      };
      (key, to_usv_str(entry.value.value))
    }))
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_none()
  }

  pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
    self.0.iter().flat_map(|entries| entries.iter().map(|(k, v)| (k.as_str(), v.as_str())))
  }

  pub fn get(&self, key: &str) -> Option<&str> {
    self.iter().find_map(|(k, v)| (k == key).then_some(v))
  }

  /// The built-in module type the `type` attribute selects. It beats the file extension and
  /// `moduleTypes`, but not the `moduleType` returned by a load hook.
  pub fn module_type(&self) -> Option<ModuleType> {
    match self.get("type")? {
      "json" => Some(ModuleType::Json),
      "text" => Some(ModuleType::Text),
      "bytes" => Some(ModuleType::Binary),
      _ => None,
    }
  }

  fn to_json(&self) -> String {
    let entries = self
      .iter()
      .map(|(key, value)| {
        concat_string!(json_escape_simd::escape(key), ":", json_escape_simd::escape(value))
      })
      .join(",");
    concat_string!("{", entries, "}")
  }

  /// `with { type: "json" }`, or `None` without attributes.
  pub fn to_with_clause(&self, ascii_only: bool) -> Option<String> {
    if self.is_empty() {
      return None;
    }
    Some(concat_string!(
      "with { ",
      self
        .iter()
        .map(|(key, value)| {
          concat_string!(
            to_module_import_export_name_with_ascii(key, ascii_only),
            ": ",
            to_ecma_string_literal(value, ascii_only)
          )
        })
        .join(", "),
      " }"
    ))
  }

  /// `rawId?attributes=<encodeURIComponent(JSON.stringify(attributes))>`, or `raw_id` itself
  /// without attributes. Byte-identical to Rollup's `generateIdByRawIdAndAttributes`.
  pub fn compose_module_id(&self, raw_id: &str) -> String {
    if self.is_empty() {
      return raw_id.to_string();
    }
    let separator = if raw_id.contains('?') { "&" } else { "?" };
    let encoded = utf8_percent_encode(&self.to_json(), URI_COMPONENT).to_string();
    concat_string!(raw_id, separator, ATTRIBUTES_MARKER, encoded)
  }

  /// Splits a module id into its raw id and attributes, like Rollup's `getRawIdAndAttributes`.
  /// The suffix only counts when composing it again gives back exactly `id`, so an id that
  /// merely looks like one stays opaque. Returns `None` when `id` carries no attributes.
  pub fn parse_module_id(id: &str) -> Option<(&str, Self)> {
    let marker = id.rfind(ATTRIBUTES_MARKER)?;
    let raw_id = id[..marker].strip_suffix(['?', '&'])?;
    let json = percent_decode_str(&id[marker + ATTRIBUTES_MARKER.len()..]).decode_utf8().ok()?;
    let entries: FxHashMap<String, String> = serde_json::from_str(&json).ok()?;
    let attributes = Self::from_entries(entries);
    (attributes.compose_module_id(raw_id) == id).then_some((raw_id, attributes))
  }
}

/// How JS orders the own string keys of an object whose properties were added in sorted order.
fn compare_keys(a: &str, b: &str) -> Ordering {
  match (as_array_index(a), as_array_index(b)) {
    (Some(a), Some(b)) => a.cmp(&b),
    (Some(_), None) => Ordering::Less,
    (None, Some(_)) => Ordering::Greater,
    // JS compares strings by UTF-16 code units, which differs from byte order outside the BMP.
    (None, None) => a.encode_utf16().cmp(b.encode_utf16()),
  }
}

/// An array index is the canonical decimal form of an integer in `0..u32::MAX`.
fn as_array_index(key: &str) -> Option<u32> {
  let canonical = key == "0"
    || (!key.is_empty() && !key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
  if !canonical {
    return None;
  }
  key.parse::<u32>().ok().filter(|index| *index != u32::MAX)
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
    // JSON.stringify('"\\\n\u0001 é') === '"\\"\\\\\\n\\u0001 é"'
    assert_eq!(
      attrs(&[("k", "\"\\\n\u{1}\u{2028}é !'()*~")]).compose_module_id("a"),
      "a?attributes=%7B%22k%22%3A%22%5C%22%5C%5C%5Cn%5Cu0001%E2%80%A8%C3%A9%20!'()*~%22%7D"
    );
  }

  #[test]
  fn last_duplicated_key_wins() {
    let attributes = attrs(&[("type", "a"), ("x", "1"), ("type", "b")]);
    assert_eq!(attributes.get("type"), Some("b"));
    assert_eq!(attributes.iter().count(), 2);
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
      "attributes=%7B%22type%22%3A%22json%22%7D",
      "a-attributes=%7B%22type%22%3A%22json%22%7D",
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
