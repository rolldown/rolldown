use std::path::Path;

/// Supported CSS preprocessor languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocessorLang {
  Scss,
  Sass,
  Less,
  Stylus,
}

impl PreprocessorLang {
  /// Detect preprocessor language from a file path based on its extension.
  /// Returns `None` for plain CSS files or unrecognized extensions.
  pub fn from_path(path: &str) -> Option<Self> {
    let ext = Path::new(path).extension()?.to_str()?;
    match ext {
      "scss" => Some(Self::Scss),
      "sass" => Some(Self::Sass),
      "less" => Some(Self::Less),
      "styl" | "stylus" => Some(Self::Stylus),
      _ => None,
    }
  }
}

/// Result of compiling a preprocessor file to CSS.
pub struct PreprocessorResult {
  /// The compiled CSS output.
  pub css: String,
  /// Paths of files that were imported/used (for watch mode).
  pub dependencies: Vec<String>,
}

/// Compile a preprocessor source file to CSS.
///
/// Currently supports Sass/SCSS via the `grass` crate (pure Rust).
/// Less and Stylus are stubbed — the source is passed through as CSS,
/// which works for the subset of Less/Stylus that is also valid CSS.
pub fn compile(
  lang: PreprocessorLang,
  source: &str,
  file_path: &str,
) -> anyhow::Result<PreprocessorResult> {
  match lang {
    PreprocessorLang::Scss | PreprocessorLang::Sass => compile_sass(source, file_path),
    PreprocessorLang::Less => Ok(compile_less_stub(source)),
    PreprocessorLang::Stylus => Ok(compile_stylus_stub(source)),
  }
}

/// Compile Sass/SCSS source to CSS using the `grass` crate.
fn compile_sass(source: &str, file_path: &str) -> anyhow::Result<PreprocessorResult> {
  let file = Path::new(file_path);
  let parent = file.parent().unwrap_or(Path::new("."));

  let options = grass::Options::default().load_path(parent);

  let css = grass::from_string(source.to_owned(), &options).map_err(|e| anyhow::anyhow!("{e}"))?;

  // grass doesn't expose dependency tracking directly, so we return empty deps.
  // The lightningcss @import inlining step will handle CSS-level @import deps.
  Ok(PreprocessorResult { css, dependencies: Vec::new() })
}

/// Stub for Less: pass source through as CSS.
///
/// Many `.less` files are valid CSS. Full Less compilation would require
/// a Node.js-based compiler or a Rust Less implementation.
fn compile_less_stub(source: &str) -> PreprocessorResult {
  PreprocessorResult { css: source.to_owned(), dependencies: Vec::new() }
}

/// Stub for Stylus: pass source through as CSS.
///
/// Many `.styl` files are valid CSS. Full Stylus compilation would require
/// a Node.js-based compiler.
fn compile_stylus_stub(source: &str) -> PreprocessorResult {
  PreprocessorResult { css: source.to_owned(), dependencies: Vec::new() }
}

/// Check if a file is a CSS module variant of a preprocessor file.
/// E.g. `foo.module.scss`, `bar.module.less`.
///
/// Returns `false` for plain CSS module files (`.module.css`), which are
/// handled by `rolldown_plugin_utils::css::is_css_module`.
pub fn is_preprocessor_css_module(path: &str) -> bool {
  // Must be a recognized preprocessor extension
  if PreprocessorLang::from_path(path).is_none() {
    return false;
  }
  let stem = Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("");
  stem.ends_with(".module")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_detect_preprocessor_lang() {
    assert_eq!(PreprocessorLang::from_path("style.scss"), Some(PreprocessorLang::Scss));
    assert_eq!(PreprocessorLang::from_path("style.sass"), Some(PreprocessorLang::Sass));
    assert_eq!(PreprocessorLang::from_path("style.less"), Some(PreprocessorLang::Less));
    assert_eq!(PreprocessorLang::from_path("style.styl"), Some(PreprocessorLang::Stylus));
    assert_eq!(PreprocessorLang::from_path("style.stylus"), Some(PreprocessorLang::Stylus));
    assert_eq!(PreprocessorLang::from_path("style.css"), None);
    assert_eq!(PreprocessorLang::from_path("style.js"), None);
  }

  #[test]
  fn test_is_preprocessor_css_module() {
    assert!(is_preprocessor_css_module("foo.module.scss"));
    assert!(is_preprocessor_css_module("bar.module.less"));
    assert!(is_preprocessor_css_module("/path/to/baz.module.styl"));
    assert!(!is_preprocessor_css_module("foo.scss"));
    assert!(!is_preprocessor_css_module("foo.module.css")); // handled by is_css_module
  }

  #[test]
  fn test_compile_scss_basic() {
    let scss = "$color: red;\n.foo { color: $color; }\n";
    let result = compile(PreprocessorLang::Scss, scss, "/tmp/test.scss").unwrap();
    assert!(result.css.contains(".foo"));
    assert!(result.css.contains("red"));
    // Should not contain SCSS variable syntax
    assert!(!result.css.contains("$color"));
  }

  #[test]
  fn test_compile_scss_nesting() {
    let scss = ".parent { .child { color: blue; } }\n";
    let result = compile(PreprocessorLang::Scss, scss, "/tmp/test.scss").unwrap();
    assert!(result.css.contains(".parent .child"));
    assert!(result.css.contains("blue"));
  }

  #[test]
  fn test_compile_less_stub() {
    let less = ".foo { color: red; }\n";
    let result = compile(PreprocessorLang::Less, less, "/tmp/test.less").unwrap();
    assert_eq!(result.css, less);
  }

  #[test]
  fn test_compile_stylus_stub() {
    let styl = ".foo { color: red; }\n";
    let result = compile(PreprocessorLang::Stylus, styl, "/tmp/test.styl").unwrap();
    assert_eq!(result.css, styl);
  }
}
