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

/// Escapes the JS line terminators in `text`. A raw one ends a `//` comment, and the rest of
/// `text` becomes code.
pub fn escape_line_terminators(text: String) -> String {
  use oxc::syntax::line_terminator::{CR, LF, LS, PS};

  // LS and PS are `E2 80 A8` and `E2 80 A9` in UTF-8. A byte scan for `0xE2` is much faster
  // than a char scan, and it finds both.
  let bytes = text.as_bytes();
  let has_line_terminator = memchr::memchr3_iter(b'\n', b'\r', 0xE2, bytes)
    .any(|i| bytes[i] != 0xE2 || matches!(bytes.get(i + 1..i + 3), Some([0x80, 0xA8 | 0xA9])));
  if !has_line_terminator {
    return text;
  }
  let mut escaped = String::with_capacity(text.len() + 8);
  for char in text.chars() {
    match char {
      LF => escaped.push_str("\\n"),
      CR => escaped.push_str("\\r"),
      LS => escaped.push_str("\\u2028"),
      PS => escaped.push_str("\\u2029"),
      _ => escaped.push(char),
    }
  }
  escaped
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

#[test]
fn test_escape_line_terminators() {
  assert_eq!(escape_line_terminators("src/main.js".to_string()), "src/main.js");
  assert_eq!(escape_line_terminators("a\u{2014}b\u{2026}".to_string()), "a\u{2014}b\u{2026}");
  assert_eq!(
    escape_line_terminators("a\nb\rc\u{2028}d\u{2029}e".to_string()),
    "a\\nb\\rc\\u2028d\\u2029e"
  );
}

#[test]
fn test_escape_line_terminators_all_unicode_scalars() {
  let text: String = (0..=u32::from(char::MAX))
    .filter_map(char::from_u32)
    .filter(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    .collect();
  let expected = text.clone();
  let ptr = text.as_ptr();
  let escaped = escape_line_terminators(text);
  assert_eq!(escaped, expected);
  // Ordinary Unicode must retain the allocation, including other E2-prefixed characters.
  assert_eq!(escaped.as_ptr(), ptr);
}

#[test]
fn test_escape_line_terminators_fuzz() {
  use oxc::{allocator::Allocator, parser::Parser, span::SourceType};

  fn check(text: String) {
    // Use independent replacements as an oracle for the byte scan and character loop.
    let expected = text
      .replace('\n', "\\n")
      .replace('\r', "\\r")
      .replace('\u{2028}', "\\u2028")
      .replace('\u{2029}', "\\u2029");
    let escaped = escape_line_terminators(text);
    assert_eq!(escaped, expected);
    assert_eq!(escape_line_terminators(escaped.clone()), escaped);

    // Cover ordinary comments, full debug headers, and HMR region markers.
    for prefix in ["// ", "//! ", "//#region "] {
      let source = format!("{prefix}{escaped}\nlet sentinel = 1;");
      let allocator = Allocator::default();
      let parsed = Parser::new(&allocator, &source, SourceType::mjs()).parse();
      assert!(parsed.diagnostics.is_empty(), "{source:?}: {:?}", parsed.diagnostics);
      assert_eq!(parsed.program.body.len(), 1, "{source:?}");
    }
  }

  let fragments = [
    "",
    "ascii",
    "\\",
    r"\n\r\u2028\u2029",
    "\0",
    "\u{85}\u{B}\u{C}",
    "\u{2027}\u{202A}",
    "—…",
    "π中文😀",
    "/* */ //",
    "'\"`",
    "//# sourceMappingURL=data:application/json,{}",
    "<!-- -->",
    "\r\n",
    "\n\r\u{2028}\u{2029}",
    "throw new Error('escaped');",
  ];
  for left in fragments {
    for right in fragments {
      for terminator in ['\n', '\r', '\u{2028}', '\u{2029}'] {
        check(format!("{left}{terminator}{right}"));
      }
    }
  }
  // Exercise every byte alignment around typical SIMD block boundaries.
  for offset in 0..=128 {
    for terminator in ['\n', '\r', '\u{2028}', '\u{2029}'] {
      check(format!("{}{terminator}—…😀\\", "a".repeat(offset)));
    }
  }
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
