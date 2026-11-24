use oxc::{
  ast::{
    Comment,
    ast::{AccessorProperty, Decorator, Program, Statement},
  },
  ast_visit::{Visit, walk},
};

/// Check if source contains `</script` (case insensitive) which requires transformer to handle tagged template literals.
pub fn contains_script_closing_tag(source: &[u8]) -> bool {
  // Use memchr to find `"</"` (start of a closing tag), then check if followed by `script` (case insensitive)
  let finder = memchr::memmem::Finder::new(b"</");
  let mut remaining = source;
  while let Some(pos) = finder.find(remaining) {
    // Check if there's enough bytes for `</script` (8 bytes)
    if remaining.len() >= pos + 8 {
      if is_script_close_tag(&remaining[pos..pos + 8]) {
        return true;
      }
    }
    // Move past the current `</` to continue searching
    remaining = &remaining[pos + 2..];
  }
  false
}

/// Whether `source` may contain standard decorator or auto-accessor syntax.
///
/// A pre-filter for skipping the transformer on plain JS, mirroring oxc's own
/// `may_contain_standard_decorator_syntax`, which only runs once the transformer
/// is already running. It skips `@` and `accessor` inside comments (JSDoc tags,
/// `@__PURE__` annotations) and `@` right after a quote (`"@scope/pkg"`), the
/// common false positives. Anything else inside strings still counts: a false
/// positive only costs a transformer pass, a false negative would leave
/// decorators unlowered.
pub fn may_contain_decorator_syntax(source: &str, comments: &[Comment]) -> bool {
  let in_comment = |position: usize| {
    let Ok(position) = u32::try_from(position) else { return false };
    let index = comments.partition_point(|comment| comment.span.end <= position);
    comments.get(index).is_some_and(|comment| comment.span.start <= position)
  };
  let bytes = source.as_bytes();
  let is_word_byte = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$');
  memchr::memchr_iter(b'@', bytes).any(|position| {
    (position == 0 || !matches!(bytes[position - 1], b'"' | b'\'' | b'`')) && !in_comment(position)
  }) || memchr::memmem::find_iter(bytes, b"accessor").any(|position| {
    let end = position + b"accessor".len();
    (position == 0 || !is_word_byte(bytes[position - 1]))
      && bytes.get(end).is_none_or(|&byte| !is_word_byte(byte))
      && !in_comment(position)
  })
}

/// Whether `program` contains a decorator or an auto-accessor.
///
/// Exact, unlike [`may_contain_decorator_syntax`]: large libraries such as
/// compilers mention `accessor` and `@` in strings often enough that a text scan
/// rarely rules them out. Run it after that scan so that most modules skip the
/// traversal; it is still far cheaper than a transformer pass, which rebuilds
/// scoping and walks the AST mutably.
pub fn contains_decorator_syntax(program: &Program<'_>) -> bool {
  struct Finder(bool);
  impl<'a> Visit<'a> for Finder {
    fn visit_statement(&mut self, it: &Statement<'a>) {
      if !self.0 {
        walk::walk_statement(self, it);
      }
    }
    fn visit_decorator(&mut self, _it: &Decorator<'a>) {
      self.0 = true;
    }
    fn visit_accessor_property(&mut self, _it: &AccessorProperty<'a>) {
      self.0 = true;
    }
  }
  let mut finder = Finder(false);
  finder.visit_program(program);
  finder.0
}

// Copy from https://github.com/oxc-project/oxc/blob/3b548dc7dea2fa49c50f8d17052a2f4c74c15a2a/crates/oxc_codegen/src/str.rs#L726-L743
#[expect(clippy::inline_always)]
#[inline(always)]
fn is_script_close_tag(slice: &[u8]) -> bool {
  // Compiler condenses these operations to an 8-byte read, u64 AND, and u64 compare.
  // https://godbolt.org/z/K8q68WGn6
  let mut bytes: [u8; 8] = slice.try_into().unwrap();
  for byte in bytes.iter_mut().skip(2) {
    // `| 32` converts ASCII upper case letters to lower case.
    *byte |= 32;
  }
  bytes == *b"</script"
}

#[cfg(test)]
mod tests {
  use super::*;

  fn scan(source: &str) -> bool {
    let allocator = oxc::allocator::Allocator::default();
    let ret = oxc::parser::Parser::new(&allocator, source, oxc::span::SourceType::mjs()).parse();
    may_contain_decorator_syntax(source, &ret.program.comments)
  }

  fn exact(source: &str) -> bool {
    let allocator = oxc::allocator::Allocator::default();
    let ret = oxc::parser::Parser::new(&allocator, source, oxc::span::SourceType::mjs()).parse();
    contains_decorator_syntax(&ret.program)
  }

  #[test]
  fn test_contains_decorator_syntax() {
    assert!(exact("@dec class A {}"));
    assert!(exact("export default @dec class {}"));
    assert!(exact("f(class { @dec m() {} })"));
    assert!(exact("class A { static accessor x = 1 }"));
    assert!(exact("function f() { return class { accessor #x } }"));
    // Text-scan false positives.
    assert!(!exact("for (const accessor of list) {} const s = 'a@b.c, a set accessor x';"));
  }

  #[test]
  fn test_may_contain_decorator_syntax() {
    assert!(scan("@dec class A {}"));
    assert!(scan("class A { accessor x = 1 }"));
    assert!(scan("class A { static accessor\nx }"));
    assert!(!scan("class A { getAccessor() {} accessorName = 1 }"));
    assert!(!scan("const $accessor = 1, accessor_ = 2;"));
    // Common false positives in npm code.
    assert!(!scan("/** @param {string} a */ export const x = /* @__PURE__ */ f(); // accessor"));
    assert!(!scan("import a from \"@scope/pkg\"; import b from '@scope/b';"));
  }

  #[test]
  fn test_contains_script_closing_tag_lowercase() {
    assert!(contains_script_closing_tag(b"</script>"));
    assert!(contains_script_closing_tag(b"some code </script> more"));
    assert!(contains_script_closing_tag(b"prefix</script"));
  }

  #[test]
  fn test_contains_script_closing_tag_uppercase() {
    assert!(contains_script_closing_tag(b"</SCRIPT>"));
    assert!(contains_script_closing_tag(b"</SCRIPT"));
    assert!(contains_script_closing_tag(b"code </SCRIPT> code"));
  }

  #[test]
  fn test_contains_script_closing_tag_mixed_case() {
    assert!(contains_script_closing_tag(b"</Script>"));
    assert!(contains_script_closing_tag(b"</ScRiPt>"));
    // spellchecker:ignore-next-line
    assert!(contains_script_closing_tag(b"</sCRIPT"));
  }

  #[test]
  fn test_contains_script_closing_tag_not_found() {
    assert!(!contains_script_closing_tag(b""));
    assert!(!contains_script_closing_tag(b"no script tag here"));
    assert!(!contains_script_closing_tag(b"<script>"));
    assert!(!contains_script_closing_tag(b"</scrip"));
    assert!(!contains_script_closing_tag(b"</ script"));
    assert!(!contains_script_closing_tag(b"</style>"));
  }

  #[test]
  fn test_contains_script_closing_tag_multiple_occurrences() {
    assert!(contains_script_closing_tag(b"</script></script>"));
    assert!(contains_script_closing_tag(b"</div></script></span>"));
  }

  #[test]
  fn test_contains_script_closing_tag_in_tagged_template() {
    assert!(contains_script_closing_tag(b"html`</script>`"));
    assert!(contains_script_closing_tag(b"const x = `</SCRIPT>`"));
  }
}
