use std::path::Path;

use rolldown_error::{BuildDiagnostic, InvalidOptionType};
use rolldown_utils::replace_all_placeholder::{ReplaceAllPlaceholder, Replacer};

use crate::ModuleId;

/// Extra context passed to [`FilenameTemplate::render`] so that an invalid `[name]` substitution
/// can be reported together with the chunk it came from. Borrowed and only materialized into owned
/// strings on the error path, so it stays cheap for the common (successful) case.
#[derive(Debug, Default, Clone, Copy)]
pub struct FilenameSubstitutionContext<'a> {
  /// Facade (entry) module id of the chunk being rendered, if any.
  pub facade_module_id: Option<&'a str>,
  /// Module ids contained in the chunk being rendered.
  pub module_ids: &'a [ModuleId],
}

/// Check if a string is a path fragment (absolute or relative path).
/// Patterns can be neither absolute nor relative paths.
///
/// Returns true if the name:
/// - Starts with "/" (Unix absolute path)
/// - Starts with "./" or "../" (relative paths)
/// - Is an absolute path (e.g., "C:/" on Windows)
pub fn is_path_fragment(name: &str) -> bool {
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
  pattern_name: &'static str,
}

impl FilenameTemplate {
  pub fn new(template: String, pattern_name: &'static str) -> Self {
    Self { template, pattern_name }
  }

  pub fn template(&self) -> &str {
    &self.template
  }

  pub fn pattern_name(&self) -> &'static str {
    self.pattern_name
  }
}

impl FilenameTemplate {
  pub fn render(
    self,
    name: Option<&str>,
    format: Option<&str>,
    extension: Option<&str>,
    hash_replacer: Option<impl Replacer>,
    context: FilenameSubstitutionContext<'_>,
  ) -> Result<String, BuildDiagnostic> {
    let pattern_name = self.pattern_name;

    // Validate the template pattern itself
    if is_path_fragment(&self.template) {
      return Err(BuildDiagnostic::invalid_option(InvalidOptionType::InvalidFilenamePattern {
        pattern: self.template,
        pattern_name: pattern_name.to_string(),
      }));
    }

    let mut tmp = self.template;

    if let Some(name) = name {
      // Validate the name replacement
      if is_path_fragment(name) {
        return Err(BuildDiagnostic::invalid_option(
          InvalidOptionType::InvalidFilenameSubstitution {
            name: name.to_string(),
            pattern_name: pattern_name.to_string(),
            facade_module_id: context.facade_module_id.map(str::to_string),
            module_ids: context.module_ids.iter().map(ToString::to_string).collect(),
          },
        ));
      }
      tmp = tmp.replace_all("[name]", name);
    }

    if let Some(format) = format {
      tmp = tmp.replace_all("[format]", format);
    }

    if let Some(hash_replacer) = hash_replacer {
      tmp = tmp.replace_all_with_len("[hash]", hash_replacer)?;
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
      FilenameTemplate::new("[name]-[hash:3]-[hash:3].js".to_string(), "entryFileNames");

    let mut hash_iter = ["abc", "def"].iter();
    let hash_replacer =
      filename_template.has_hash_pattern().then_some(|_| Ok(hash_iter.next().unwrap()));

    let filename = filename_template
      .render(Some("hello"), None, None, hash_replacer, FilenameSubstitutionContext::default())
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
    let template = FilenameTemplate::new("/absolute/path/[name].js".to_string(), "entryFileNames");
    let result = template.render(
      Some("test"),
      None,
      None,
      None::<&str>,
      FilenameSubstitutionContext::default(),
    );
    assert!(result.is_err());
    assert!(
      result.unwrap_err().to_string().contains("patterns can be neither absolute nor relative")
    );
  }

  #[test]
  fn test_invalid_name_substitution() {
    let template = FilenameTemplate::new("[name].js".to_string(), "entryFileNames");
    let result = template.render(
      Some("/absolute/name"),
      None,
      None,
      None::<&str>,
      FilenameSubstitutionContext::default(),
    );
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Invalid substitution \"/absolute/name\" for placeholder \"[name]\"")
    );
  }

  #[test]
  fn invalid_name_substitution_reports_chunk_context() {
    let template = FilenameTemplate::new("[name].js".to_string(), "chunkFileNames");
    let facade: ModuleId = "/project/node_modules/dep/index.js".into();
    let modules: Vec<ModuleId> = vec![facade.clone(), "/project/node_modules/dep/util.js".into()];
    let context =
      FilenameSubstitutionContext { facade_module_id: Some(facade.as_str()), module_ids: &modules };
    let result =
      template.render(Some("../node_modules/dep/index"), None, None, None::<&str>, context);
    let err = result.unwrap_err().to_string();
    // The bare substitution error is still present...
    assert!(err.contains("Invalid substitution \"../node_modules/dep/index\""));
    // ...plus the chunk context that points users at the source module.
    assert!(err.contains("derived from module: /project/node_modules/dep/index.js"));
    assert!(err.contains("This chunk contains modules:"));
  }

  #[test]
  fn test_valid_subdirectory() {
    let template = FilenameTemplate::new("dist/[name].js".to_string(), "entryFileNames");
    let result = template.render(
      Some("test"),
      None,
      None,
      None::<&str>,
      FilenameSubstitutionContext::default(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "dist/test.js");
  }
}
