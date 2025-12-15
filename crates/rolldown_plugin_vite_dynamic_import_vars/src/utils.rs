use std::path::Path;

use rolldown_utils::pattern_filter::filter as pattern_filter;

use super::ViteDynamicImportVarsPlugin;

impl ViteDynamicImportVarsPlugin {
  pub fn filter(&self, id: &str, cwd: &Path) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return true;
    }

    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());
    pattern_filter(exclude, include, id, &cwd.to_string_lossy()).inner()
  }
}

/// Check if code contains dynamic import pattern: /\bimport\s*[(/]/
/// This is a fast check to avoid parsing files that don't contain dynamic imports.
pub fn has_dynamic_import(code: &str) -> bool {
  let bytes = code.as_bytes();

  let mut i = 0;
  while i + 6 < bytes.len() {
    // Find "import"
    if let Some(pos) = memchr::memmem::find(&bytes[i..], b"import") {
      let abs_pos = i + pos;

      // Check word boundary before "import"
      if abs_pos > 0 {
        let prev = bytes[abs_pos - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
          i = abs_pos + 6;
          continue;
        }
      }

      // Skip whitespace after "import"
      let mut j = abs_pos + 6;
      while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
      }

      // Check for '('
      if j < bytes.len() && bytes[j] == b'(' {
        // Skip whitespace after "("
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        // Check for template literal
        if j < bytes.len() && bytes[j] == b'`' {
          return true;
        }
      }

      i = j;
    } else {
      break;
    }
  }
  false
}
