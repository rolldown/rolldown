use std::collections::HashMap;

use lightningcss::{
  css_modules::{self, CssModuleExport, CssModuleExports, CssModuleReference},
  printer::PrinterOptions,
  stylesheet::{ParserOptions, StyleSheet},
};

/// Result of processing a CSS module file.
pub struct CssModuleResult {
  /// The transformed CSS with hashed class names.
  pub code: String,
  /// The JS proxy source with named exports for each class name.
  pub js_proxy: String,
  /// The module exports map (original name -> hashed name + composes).
  pub exports: CssModuleExports,
}

/// Process a CSS module file: parse with CSS modules enabled, extract exports,
/// and generate a JS proxy module with named exports.
pub fn transform_css_module(file_id: &str, css_code: &str) -> anyhow::Result<CssModuleResult> {
  let stylesheet = StyleSheet::parse(
    css_code,
    ParserOptions {
      filename: file_id.to_owned(),
      css_modules: Some(css_modules::Config {
        pattern: css_modules::Pattern::parse("[hash]_[local]")
          .map_err(|e| anyhow::anyhow!("CSS modules pattern error: {e}"))?,
        dashed_idents: false,
        ..Default::default()
      }),
      ..Default::default()
    },
  )
  .map_err(|e| anyhow::anyhow!("CSS parse error in {file_id}: {e}"))?;

  let result = stylesheet
    .to_css(PrinterOptions::default())
    .map_err(|e| anyhow::anyhow!("CSS printer error: {e}"))?;

  let exports = result.exports.unwrap_or_default();
  let js_proxy = generate_js_proxy(file_id, &exports);

  Ok(CssModuleResult { code: result.code, js_proxy, exports })
}

/// Generate JS proxy source code with named exports for each CSS module export.
///
/// Output example:
/// ```js
/// export const container = "x7y8z9_container";
/// export const title = "a1b2c3_title";
/// ```
fn generate_js_proxy(file_id: &str, exports: &CssModuleExports) -> String {
  // Sort keys alphabetically for deterministic output
  let mut keys: Vec<&String> = exports.keys().collect();
  keys.sort();

  let mut lines = Vec::with_capacity(keys.len() + 2);
  lines.push(format!("// CSS modules proxy for {file_id}"));

  // Collect all export values, then build `export default` and named exports
  let mut default_entries: Vec<(String, String)> = Vec::new();

  for key in &keys {
    let export = &exports[*key];
    let value = build_export_value(export);
    let safe_name = sanitize_ident(key);
    lines.push(format!("export const {safe_name} = \"{value}\";"));
    default_entries.push(((*key).clone(), value));
  }

  // Build default export as an object mapping original names to values
  if default_entries.is_empty() {
    lines.push("export default {};".to_owned());
  } else {
    let entries: Vec<String> = default_entries
      .iter()
      .map(|(key, val)| {
        let safe = sanitize_ident(key);
        // If original key differs from sanitized, use quoted key
        if *key == safe {
          format!("  {safe}: \"{val}\"")
        } else {
          format!("  \"{key}\": \"{val}\"")
        }
      })
      .collect();
    lines.push(format!("export default {{\n{}\n}};", entries.join(",\n")));
  }

  lines.push(String::new()); // trailing newline
  lines.join("\n")
}

/// Build the runtime value string for a CSS module export, including composed references.
fn build_export_value(export: &CssModuleExport) -> String {
  let mut parts = vec![export.name.clone()];

  for reference in &export.composes {
    match reference {
      CssModuleReference::Local { name }
      | CssModuleReference::Global { name }
      | CssModuleReference::Dependency { name, specifier: _ } => {
        parts.push(name.clone());
      }
    }
  }

  parts.join(" ")
}

/// Sanitize a CSS class name to be a valid JS identifier.
///
/// - Replace hyphens with underscores
/// - Prefix with `_` if starting with a digit
fn sanitize_ident(name: &str) -> String {
  let sanitized: String = name.chars().map(|c| if c == '-' { '_' } else { c }).collect();

  if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
    format!("_{sanitized}")
  } else {
    sanitized
  }
}

/// Shared state: cache of CSS module exports per module ID.
#[derive(Debug, Default)]
pub struct CssModulesExportsCache {
  pub inner: HashMap<String, CssModuleExports>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_sanitize_ident() {
    assert_eq!(sanitize_ident("foo-bar"), "foo_bar");
    assert_eq!(sanitize_ident("123abc"), "_123abc");
    assert_eq!(sanitize_ident("normal"), "normal");
  }

  #[test]
  fn test_build_export_value_simple() {
    let export =
      CssModuleExport { name: "abc_container".to_owned(), composes: vec![], is_referenced: true };
    assert_eq!(build_export_value(&export), "abc_container");
  }

  #[test]
  fn test_build_export_value_with_composes() {
    let export = CssModuleExport {
      name: "abc_container".to_owned(),
      composes: vec![
        CssModuleReference::Local { name: "xyz_base".to_owned() },
        CssModuleReference::Global { name: "global-class".to_owned() },
      ],
      is_referenced: true,
    };
    assert_eq!(build_export_value(&export), "abc_container xyz_base global-class");
  }
}
