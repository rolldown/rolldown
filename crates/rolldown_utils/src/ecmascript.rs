use oxc::syntax::{identifier, keyword};
use oxc_str::{CompactStr, JSStr};
use std::{borrow::Cow, fmt::Write, path::Path};

use crate::{concat_string, indexmap::FxIndexMap};

#[inline]
pub fn is_validate_identifier_name(name: &str) -> bool {
  identifier::is_identifier_name(name)
}

#[inline]
pub fn is_validate_assignee_identifier_name(name: &str) -> bool {
  identifier::is_identifier_name(name) && none_preserved_keyword_or_global_object_ext(name)
}

#[inline]
/// extra branches are used to avoid SyntaxError in strict mode
/// ```bash
/// SyntaxError: Unexpected eval or arguments in strict mode
/// ```
pub fn none_preserved_keyword_or_global_object_ext(name: &str) -> bool {
  !keyword::is_reserved_keyword_or_global_object(name) && name != "arguments" && name != "eval"
}

pub fn legitimize_json_local_binding_name(
  name: &str,
  map: &FxIndexMap<CompactStr, (CompactStr, bool)>,
) -> CompactStr {
  let mut name = CompactStr::new(name);
  let none_preserved_keyword_or_global_object_binding =
    none_preserved_keyword_or_global_object_ext(&name);
  let is_identifier_name = identifier::is_identifier_name(&name);
  let exists_in_map = map.contains_key(&name);
  if is_identifier_name && none_preserved_keyword_or_global_object_binding && !exists_in_map {
    return name;
  }
  if !none_preserved_keyword_or_global_object_binding {
    loop {
      name = CompactStr::from(format!("_{name}"));
      if none_preserved_keyword_or_global_object_ext(&name) {
        break;
      }
    }
  }
  let mut name = CompactStr::new(&legitimize_identifier_name(&name));
  loop {
    if !map.contains_key(&name) {
      break;
    }
    name = CompactStr::from(format!("_{name}"));
  }
  name
}

pub fn legitimize_identifier_name(name: &str) -> Cow<'_, str> {
  if name.is_empty() {
    return Cow::Borrowed("_");
  }

  let mut chars_indices = name.char_indices();

  let mut first_invalid_char_index = None;

  if let Some((idx, first_char)) = chars_indices.next() {
    if !identifier::is_identifier_start(first_char) {
      first_invalid_char_index = Some(idx);
    }
  }

  if first_invalid_char_index.is_none() {
    first_invalid_char_index =
      chars_indices.find(|(_idx, char)| !identifier::is_identifier_part(*char)).map(|(idx, _)| idx);
  }

  let Some(first_invalid_char_index) = first_invalid_char_index else {
    return Cow::Borrowed(name);
  };

  // Allocate only on the path that actually legitimizes the name.
  let mut legitimized = String::with_capacity(name.len());
  let (first_valid_part, rest_part) = name.split_at(first_invalid_char_index);
  legitimized.push_str(first_valid_part);
  if first_invalid_char_index == 0 {
    legitimized.push('_');
  }
  for (i, char) in rest_part.chars().enumerate() {
    if identifier::is_identifier_part(char) {
      legitimized.push(char);
    } else {
      // Avoid generating duplicate `_` for the first char
      if i != 0 || first_invalid_char_index != 0 {
        legitimized.push('_');
      }
    }
  }

  Cow::Owned(legitimized)
}

pub fn property_access_str(obj: &str, prop: &str) -> String {
  property_access_str_with_ascii(obj, prop, false)
}

pub fn property_access_str_with_ascii(obj: &str, prop: &str, ascii_only: bool) -> String {
  if is_validate_identifier_name(prop) {
    concat_string!(obj, ".", escape_non_ascii(prop, ascii_only))
  } else {
    concat_string!(obj, "[", to_ecma_string_literal(prop, ascii_only), "]")
  }
}

pub fn escape_non_ascii(value: &str, ascii_only: bool) -> Cow<'_, str> {
  if !ascii_only || value.is_ascii() {
    return Cow::Borrowed(value);
  }

  let mut escaped = String::with_capacity(value.len());
  for char in value.chars() {
    if char.is_ascii() {
      escaped.push(char);
    } else if u32::from(char) <= 0xffff {
      write!(escaped, "\\u{:04X}", u32::from(char)).unwrap();
    } else {
      write!(escaped, "\\u{{{:X}}}", u32::from(char)).unwrap();
    }
  }
  Cow::Owned(escaped)
}

pub fn to_ecma_string_literal(value: &str, ascii_only: bool) -> String {
  let literal = json_escape_simd::escape(value);
  escape_non_ascii(&literal, ascii_only).into_owned()
}

pub fn to_module_import_export_name_with_ascii(name: &str, ascii_only: bool) -> String {
  if is_validate_identifier_name(name) {
    escape_non_ascii(name, ascii_only).into_owned()
  } else {
    to_ecma_string_literal(name, ascii_only)
  }
}

#[test]
fn test_is_validate_identifier_name() {
  assert!(is_validate_identifier_name("foo"));
  assert!(!is_validate_identifier_name("1aaaa"));
  assert!(!is_validate_identifier_name("😈"));
}

#[test]
fn test_legitimize_identifier_name() {
  assert_eq!(legitimize_identifier_name("foo"), "foo");
  assert_eq!(legitimize_identifier_name("@babel/types"), "_babel_types");
  assert_eq!(legitimize_identifier_name("$foo$"), "$foo$");
  assert_eq!(legitimize_identifier_name("react-dom"), "react_dom");
  assert_eq!(legitimize_identifier_name("111a"), "_111a");
}

#[test]
fn test_escape_non_ascii() {
  assert_eq!(escape_non_ascii("π你😀", false), "π你😀");
  assert_eq!(escape_non_ascii("π你😀", true), "\\u03C0\\u4F60\\u{1F600}");
  assert_eq!(to_ecma_string_literal("外部", true), "\"\\u5916\\u90E8\"");
}

/// Converts a JavaScript string to UTF-8 the way `URL` and Node's file system APIs do: each lone
/// surrogate becomes U+FFFD. A module request with a lone surrogate then names the file the runtime
/// would load.
pub fn to_usv_str(value: JSStr<'_>) -> Cow<'_, str> {
  match value.as_str() {
    Some(value) => Cow::Borrowed(value),
    None => Cow::Owned(
      char::decode_utf16(value.encode_utf16())
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect(),
    ),
  }
}

#[test]
fn test_to_usv_str() {
  use oxc::{allocator::Allocator, ast::ast::Expression, parser::Parser, span::SourceType};
  let allocator = Allocator::default();
  let expr = Parser::new(&allocator, r"'a\uD800b\uD800\uDC00'", SourceType::default())
    .parse_expression()
    .unwrap();
  let Expression::StringLiteral(lit) = expr else { unreachable!() };
  // A lone surrogate becomes U+FFFD, and a pair stays one code point.
  assert_eq!(to_usv_str(lit.value), "a\u{FFFD}b\u{10000}");
  assert!(matches!(to_usv_str(JSStr::from("plain")), Cow::Borrowed("plain")));
}

pub fn is_relative_specifier(specifier: &str) -> bool {
  // `Path::is_relative` is not used here because it consider implicit relative path as relative path. such
  // as `Path::new("foo.txt")` is considered as a relative path.
  specifier.starts_with("./") || specifier.starts_with("../")
}

/// Check if the specifier is a path-like specifier. E.g. `./foo`, `../foo`, `/foo`, `C:\foo`, `file:///foo`
pub fn is_path_like_specifier(specifier: &str) -> bool {
  is_relative_specifier(specifier)
    || Path::new(specifier).is_absolute()
    || specifier.starts_with('/') // Though starting with `/` is not a absolute path in Windows, we still consider it as a path-like specifier.
}
