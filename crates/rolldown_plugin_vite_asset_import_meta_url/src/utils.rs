pub const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

/// Checks if the code contains `new URL('...', import.meta.url)` pattern
/// where first parameter must be a string literal and second is import.meta.url
pub fn contains_asset_import_meta_url(code: &str) -> bool {
  let bytes = code.as_bytes();
  let len = bytes.len();
  let mut i = 0;

  while i < len {
    if i + 3 <= len && &bytes[i..i + 3] == b"new" {
      let mut j = i + 3;
      while j < len && bytes[j].is_ascii_whitespace() {
        j += 1;
      }

      if j + 3 <= len && &bytes[j..j + 3] == b"URL" {
        j += 3;
        while j < len && bytes[j].is_ascii_whitespace() {
          j += 1;
        }

        // Must have opening parenthesis
        if j >= len || bytes[j] != b'(' {
          i += 1;
          continue;
        }
        j += 1;

        // Skip whitespace before first parameter
        while j < len && bytes[j].is_ascii_whitespace() {
          j += 1;
        }

        // First parameter must be a string literal
        if j >= len || !matches!(bytes[j], b'\'' | b'"' | b'`') {
          i += 1;
          continue;
        }

        // Skip the string literal
        let quote = bytes[j];
        j += 1;
        while j < len {
          if bytes[j] == b'\\' {
            j += 2; // Skip escaped character
          } else if bytes[j] == quote {
            j += 1; // Skip closing quote
            break;
          } else {
            j += 1;
          }
        }

        // Skip whitespace after string
        while j < len && bytes[j].is_ascii_whitespace() {
          j += 1;
        }

        // Must have comma
        if j >= len || bytes[j] != b',' {
          i += 1;
          continue;
        }
        j += 1; // Skip comma

        // Skip whitespace before second parameter
        while j < len && bytes[j].is_ascii_whitespace() {
          j += 1;
        }

        // Second parameter must be exactly "import.meta.url"
        if j + 15 <= len && &bytes[j..j + 15] == b"import.meta.url" {
          j += 15;

          // Skip whitespace after import.meta.url
          while j < len && bytes[j].is_ascii_whitespace() {
            j += 1;
          }

          // Must be followed by ')' or ',' (optional third parameter)
          if j < len && (bytes[j] == b')' || bytes[j] == b',') {
            return true;
          }
        }
      }
    }
    i += 1;
  }

  false
}

/// Splits a raw URL into pure URL and query string.
///
/// Returns a tuple of (pure_url, query_string):
/// - If query delimiter is found: pure_url is the part before '?', query_string is from '?' to the second-to-last character
/// - If no query delimiter: pure_url is the original URL and query_string is empty
///
/// Note: The query delimiter '?' is searched ignoring '?' characters inside curly braces.
pub fn split_url_and_query(raw_url: &str) -> (&str, &str) {
  let bytes = raw_url.as_bytes();
  let mut brackets_stack = 0;

  // Find the index of query delimiter '?' outside curly braces
  for (i, &byte) in bytes.iter().enumerate() {
    match byte {
      b'{' => brackets_stack += 1,
      b'}' => brackets_stack -= 1,
      b'?' if brackets_stack == 0 => {
        let pure_url = &raw_url[..i];
        let query_string = &raw_url[i..];
        return (pure_url, query_string);
      }
      _ => {}
    }
  }

  (raw_url, "")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic() {
    assert!(contains_asset_import_meta_url("new URL('./file.png', import.meta.url)"));
  }

  #[test]
  fn test_multiline() {
    assert!(contains_asset_import_meta_url("new URL(\n  './file.png',\n  import.meta.url\n)"));
  }

  #[test]
  fn test_no_match() {
    assert!(!contains_asset_import_meta_url("new URL('./file.png')"));
    assert!(!contains_asset_import_meta_url("const x = 123"));
  }

  #[test]
  fn test_false_positive_prevention() {
    // import.meta.url in different statement - should NOT match
    assert!(!contains_asset_import_meta_url("new URL('')\nconsole.log(import.meta.url)"));

    // import.meta.url on separate line - should NOT match
    assert!(!contains_asset_import_meta_url("new URL('test'); const x = import.meta.url"));

    // First param not a string literal - should NOT match
    assert!(!contains_asset_import_meta_url("new URL(getPath(), import.meta.url)"));

    // First param is variable - should NOT match
    assert!(!contains_asset_import_meta_url("new URL(path, import.meta.url)"));

    // import.meta.url as first parameter - should NOT match
    assert!(!contains_asset_import_meta_url("new URL(import.meta.url)"));

    // Second param is not exactly import.meta.url - should NOT match
    assert!(!contains_asset_import_meta_url("new URL('./path', import.meta.url.href)"));
    assert!(!contains_asset_import_meta_url("new URL('./path', import.meta.url || fallback)"));
    assert!(!contains_asset_import_meta_url("new URL('./path', String(import.meta.url))"));
  }

  #[test]
  fn test_string_literal_first_parameter() {
    // Single quotes
    assert!(contains_asset_import_meta_url("new URL('./path', import.meta.url)"));

    // Double quotes
    assert!(contains_asset_import_meta_url(r#"new URL("./path", import.meta.url)"#));

    // Template literal
    assert!(contains_asset_import_meta_url("new URL(`./path`, import.meta.url)"));

    // String with comma
    assert!(contains_asset_import_meta_url("new URL('a,b,c', import.meta.url)"));

    // String with escaped quotes
    assert!(contains_asset_import_meta_url(r#"new URL("path\'s file", import.meta.url)"#));

    // Template literal with interpolation
    assert!(contains_asset_import_meta_url("new URL(`./assets/${name}`, import.meta.url)"));
  }

  #[test]
  fn test_whitespace_variations() {
    assert!(contains_asset_import_meta_url("new URL('./a',import.meta.url)"));
    assert!(contains_asset_import_meta_url("new URL('./a'  ,  import.meta.url)"));
    assert!(contains_asset_import_meta_url("new  URL  (  './a'  ,  import.meta.url  )"));
  }
}
