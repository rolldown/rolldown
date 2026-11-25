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
