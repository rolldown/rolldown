use std::path::Path;

use rolldown_utils::replace_all_placeholder::{ReplaceAllPlaceholder, Replacer};

/// Check if a string is a path fragment (absolute or relative path).
/// Patterns can be neither absolute nor relative paths.
///
/// Returns true if the name:
/// - Starts with "/" (Unix absolute path)
/// - Starts with "./" or "../" (relative paths)
/// - Is an absolute path (e.g., "C:/" on Windows)
fn is_path_fragment(name: &str) -> bool {
  if name.is_empty() {
    return false;
  }

  // Check for "/" prefix (Unix absolute)
  if name.starts_with('/') {
    return true;
  }

  // Check for "./" or "../" prefix (relative)
  if name.starts_with("./") || name.starts_with("../") {
    return true;
  }

  // Check if it's an absolute path (handles Windows paths like "C:/")
  Path::new(name).is_absolute()
}

// Constants for hash pattern parsing
const HASH_PREFIX: &str = "[hash";
const HASH_PREFIX_LEN: usize = 5; // Length of "[hash"
const HASH_BRACKET_LEN: usize = 6; // Length of "[hash]" - used for skipping patterns

#[derive(Debug)]
pub struct FilenameTemplate {
  template: String,
  pattern_name: &'static str,
}

impl FilenameTemplate {
  pub fn new(template: String, pattern_name: &'static str) -> Self {
    Self { template, pattern_name }
  }

  pub fn template(&self) -> &str {
    &self.template
  }

  pub fn pattern_name(&self) -> &str {
    self.pattern_name
  }

  /// Extracts hash lengths from [hash:N] patterns in the template.
  /// Returns a vector of lengths, or None for [hash] without a length.
  fn extract_hash_lengths(&self) -> Vec<Option<usize>> {
    let mut lengths = Vec::new();
    let mut start = 0;

    while let Some(pos) = self.template[start..].find(HASH_PREFIX) {
      let pos = start + pos;
      let rest = &self.template[pos + HASH_PREFIX_LEN..];

      if let Some(&b':') = rest.as_bytes().first() {
        if let Some(end_bracket) = rest.find(']') {
          if let Ok(len) = rest[1..end_bracket].parse::<usize>() {
            lengths.push(Some(len));
            start = pos + HASH_PREFIX_LEN + end_bracket + 1;
            continue;
          }
        }
        // Malformed pattern like [hash:abc] or [hash: without closing bracket
        // Skip past "[hash:" to avoid infinite loop
        start = pos + HASH_BRACKET_LEN;
      } else if rest.starts_with(']') {
        lengths.push(None);
        // Skip past "[hash]"
        start = pos + HASH_BRACKET_LEN;
      } else {
        // Not a valid hash pattern (e.g., [hashmap])
        // Skip past "[hash" to continue searching
        start = pos + HASH_BRACKET_LEN;
      }
    }

    lengths
  }

  /// Validates hash lengths for entry and chunk file names.
  /// Returns an error if any hash length is below the minimum required (6).
  pub fn validate_hash_lengths(&self) -> anyhow::Result<()> {
    // Only validate for entry and chunk file names, not asset file names
    let requires_min_hash = matches!(
      self.pattern_name,
      "entryFileNames" | "chunkFileNames" | "cssEntryFileNames" | "cssChunkFileNames"
    );

    if !requires_min_hash {
      return Ok(());
    }

    const MIN_HASH_LENGTH: usize = 6;
    let hash_lengths = self.extract_hash_lengths();

    for hash_length in hash_lengths {
      if let Some(len) = hash_length {
        if len < MIN_HASH_LENGTH {
          anyhow::bail!(
            "Hashes in \"{}\" must be at least {} characters, received {}.",
            self.pattern_name,
            MIN_HASH_LENGTH,
            len
          );
        }
      }
    }

    Ok(())
  }
}

impl FilenameTemplate {
  pub fn render(
    self,
    name: Option<&str>,
    format: Option<&str>,
    extension: Option<&str>,
    hash_replacer: Option<impl Replacer>,
  ) -> anyhow::Result<String> {
    let pattern_name = &self.pattern_name;

    // Validate the template pattern itself
    if is_path_fragment(&self.template) {
      anyhow::bail!(
        "Invalid pattern \"{}\" for \"{}\", patterns can be neither absolute nor relative paths. \
         If you want your files to be stored in a subdirectory, write its name without a leading \
         slash like this: subdirectory/pattern.",
        self.template,
        pattern_name
      );
    }

    // Validate hash lengths for entry/chunk file names
    self.validate_hash_lengths()?;

    let mut tmp = self.template;

    if let Some(name) = name {
      // Validate the name replacement
      if is_path_fragment(name) {
        anyhow::bail!(
          "Invalid substitution \"{name}\" for placeholder \"[name]\" in \"{pattern_name}\" pattern, \
           can be neither absolute nor relative path."
        );
      }
      tmp = tmp.replace_all("[name]", name);
    }

    if let Some(format) = format {
      tmp = tmp.replace_all("[format]", format);
    }

    if let Some(hash_replacer) = hash_replacer {
      tmp = tmp.replace_all_with_len("[hash]", hash_replacer);
    }

    if let Some(ext) = extension {
      let extname = if ext.is_empty() { "" } else { &format!(".{ext}") };
      tmp = tmp.replace_all("[ext]", ext);
      tmp = tmp.replace_all("[extname]", extname);
    }

    Ok(tmp)
  }

  pub fn has_hash_pattern(&self) -> bool {
    let start = self.template.find("[hash");
    start.is_some_and(|start| {
      let pattern = &self.template[start + 5..];
      pattern.starts_with(']') || (pattern.starts_with(':') && pattern.contains(']'))
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic() {
    FilenameTemplate::new("[name]-[hash:8].js".to_string(), "entryFileNames");
  }

  #[test]
  fn hash_with_len() {
    let filename_template =
      FilenameTemplate::new("[name]-[hash:8]-[hash:7].js".to_string(), "entryFileNames");

    let mut hash_iter = ["abcdefgh", "1234567"].iter();
    let hash_replacer =
      filename_template.has_hash_pattern().then_some(|_| hash_iter.next().unwrap());

    let filename =
      filename_template.render(Some("hello"), None, None, hash_replacer).expect("should render");

    assert_eq!(filename, "hello-abcdefgh-1234567.js");
  }

  #[test]
  fn test_is_path_fragment() {
    // Absolute paths
    assert!(is_path_fragment("/absolute/path"));
    assert!(is_path_fragment("/"));

    // Relative paths
    assert!(is_path_fragment("./relative"));
    assert!(is_path_fragment("../parent"));

    // Valid subdirectory patterns (not path fragments)
    assert!(!is_path_fragment("dist/[name].js"));
    assert!(!is_path_fragment("[name]-[hash].js"));
    assert!(!is_path_fragment("chunk"));

    // Empty string
    assert!(!is_path_fragment(""));
  }

  #[test]
  fn test_invalid_pattern() {
    let template = FilenameTemplate::new("/absolute/path/[name].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, None::<&str>);
    assert!(result.is_err());
    assert!(
      result.unwrap_err().to_string().contains("patterns can be neither absolute nor relative")
    );
  }

  #[test]
  fn test_invalid_name_substitution() {
    let template = FilenameTemplate::new("[name].js".to_string(), "entryFileNames");
    let result = template.render(Some("/absolute/name"), None, None, None::<&str>);
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Invalid substitution \"/absolute/name\" for placeholder \"[name]\"")
    );
  }

  #[test]
  fn test_valid_subdirectory() {
    let template = FilenameTemplate::new("dist/[name].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, None::<&str>);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "dist/test.js");
  }

  #[test]
  fn test_hash_length_below_minimum_for_entry_filenames() {
    let template = FilenameTemplate::new("[name]-[hash:2].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, Some("abc"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be at least 6 characters"));
  }

  #[test]
  fn test_hash_length_below_minimum_for_chunk_filenames() {
    let template = FilenameTemplate::new("[name]-[hash:5].js".to_string(), "chunkFileNames");
    let result = template.render(Some("test"), None, None, Some("abc"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be at least 6 characters"));
  }

  #[test]
  fn test_hash_length_at_minimum_for_entry_filenames() {
    let template = FilenameTemplate::new("[name]-[hash:6].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, Some("abcdef"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test-abcdef.js");
  }

  #[test]
  fn test_hash_length_below_minimum_allowed_for_asset_filenames() {
    // Asset filenames should allow hash lengths below 6
    let template = FilenameTemplate::new("[name]-[hash:2].js".to_string(), "assetFileNames");
    let result = template.render(Some("test"), None, None, Some("ab"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "test-ab.js");
  }

  #[test]
  fn test_multiple_hash_patterns_with_invalid_length() {
    let template = FilenameTemplate::new("[name]-[hash:8]-[hash:3].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, Some("abc"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be at least 6 characters"));
  }

  #[test]
  fn test_malformed_hash_pattern_does_not_cause_error() {
    // Malformed patterns like [hash:abc] should not cause validation errors
    let template = FilenameTemplate::new("[name]-[hash:abc]-[hash:8].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, Some("abcdefgh"));
    assert!(result.is_ok());
  }

  #[test]
  fn test_non_hash_pattern_does_not_cause_error() {
    // Patterns like [hashmap] should not be treated as hash patterns
    let template = FilenameTemplate::new("[name]-[hashmap]-[hash:8].js".to_string(), "entryFileNames");
    let result = template.render(Some("test"), None, None, Some("abcdefgh"));
    assert!(result.is_ok());
  }
}
