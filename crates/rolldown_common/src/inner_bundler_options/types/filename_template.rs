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

#[derive(Debug)]
pub struct FilenameTemplate {
  template: String,
}

impl FilenameTemplate {
  pub fn new(template: String) -> Self {
    Self { template }
  }

  pub fn template(&self) -> &str {
    &self.template
  }
}

impl From<String> for FilenameTemplate {
  fn from(template: String) -> Self {
    Self::new(template)
  }
}

impl FilenameTemplate {
  pub fn render(
    self,
    pattern_name: &str,
    name: Option<&str>,
    format: Option<&str>,
    extension: Option<&str>,
    hash_replacer: Option<impl Replacer>,
  ) -> anyhow::Result<String> {
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
      // Validate the format replacement
      if is_path_fragment(format) {
        anyhow::bail!(
          "Invalid substitution \"{format}\" for placeholder \"[format]\" in \"{pattern_name}\" pattern, \
           can be neither absolute nor relative path."
        );
      }
      tmp = tmp.replace_all("[format]", format);
    }

    if let Some(hash_replacer) = hash_replacer {
      tmp = tmp.replace_all_with_len("[hash]", hash_replacer);
    }

    if let Some(ext) = extension {
      // Validate the extension replacement
      if is_path_fragment(ext) {
        anyhow::bail!(
          "Invalid substitution \"{ext}\" for placeholder \"[ext]\" in \"{pattern_name}\" pattern, \
           can be neither absolute nor relative path."
        );
      }
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
    FilenameTemplate::new("[name]-[hash:8].js".to_string());
  }

  #[test]
  fn hash_with_len() {
    let filename_template = FilenameTemplate::new("[name]-[hash:3]-[hash:3].js".to_string());

    let mut hash_iter = ["abc", "def"].iter();
    let hash_replacer =
      filename_template.has_hash_pattern().then_some(|_| hash_iter.next().unwrap());

    let filename = filename_template
      .render("entryFileNames", Some("hello"), None, None, hash_replacer)
      .expect("should render");

    assert_eq!(filename, "hello-abc-def.js");
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
    let template = FilenameTemplate::new("/absolute/path/[name].js".to_string());
    let result = template.render("entryFileNames", Some("test"), None, None, None::<&str>);
    assert!(result.is_err());
    assert!(
      result.unwrap_err().to_string().contains("patterns can be neither absolute nor relative")
    );
  }

  #[test]
  fn test_invalid_name_substitution() {
    let template = FilenameTemplate::new("[name].js".to_string());
    let result =
      template.render("entryFileNames", Some("/absolute/name"), None, None, None::<&str>);
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Invalid substitution \"/absolute/name\" for placeholder \"[name]\"")
    );
  }

  #[test]
  fn test_invalid_format_substitution() {
    let template = FilenameTemplate::new("[name]-[format].js".to_string());
    let result =
      template.render("entryFileNames", Some("test"), Some("./relative"), None, None::<&str>);
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Invalid substitution \"./relative\" for placeholder \"[format]\"")
    );
  }

  #[test]
  fn test_valid_subdirectory() {
    let template = FilenameTemplate::new("dist/[name].js".to_string());
    let result = template.render("entryFileNames", Some("test"), None, None, None::<&str>);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "dist/test.js");
  }
}
