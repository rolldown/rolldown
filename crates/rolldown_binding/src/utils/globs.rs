use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use rolldown_error::BuildError;
use wax::{Any, Glob};

// Convert ?(pattern) groups to <pattern:#,#> alternatives
pub static EXT_GROUPS: Lazy<Regex> =
  Lazy::new(|| Regex::new("(@|\\*|\\+|\\?|!)\\(([^)]+)\\)").unwrap());

// Convert POSIX expressions to char classes
pub static POSIX_EXPRS: Lazy<Vec<(String, String)>> = Lazy::new(|| {
  vec![
    ("[:alnum:]".into(), "[a-zA-Z0-9]".into()),
    ("[:alpha:]".into(), "[a-zA-Z]".into()),
    ("[:ascii:]".into(), "[\\x00-\\x7F]".into()),
    ("[:blank:]".into(), "[ \\t]".into()),
    ("[:cntrl:]".into(), "[\\x00-\\x1F\\x7F]".into()),
    ("[:digit:]".into(), "[0-9]".into()),
    ("[:graph:]".into(), "[\\x21-\\x7E]".into()),
    ("[:lower:]".into(), "[a-z]".into()),
    ("[:print:]".into(), "[\\x20-\\x7E ]".into()),
    ("[:punct:]".into(), "[\\-!\"#$%&\'()\\*+,./:;<=>?@[\\]^_{|}~]".into()),
    ("[:space:]".into(), "[ \\t\\r\\n\\v\\f]".into()),
    ("[:upper:]".into(), "[A-Z]".into()),
    ("[:word:]".into(), "[A-Za-z0-9_]".into()),
    ("[:xdigit:]".into(), "[A-Fa-f0-9]".into()),
  ]
});

// JS uses picomatch: https://github.com/micromatch/picomatch
// Rust uses wax: https://docs.rs/wax/0.6.0/wax/
fn convert_js_to_rust(base_pattern: &str) -> Result<String, BuildError> {
  // Always use Unix separators
  let mut pattern = base_pattern.replace('\\', "/");

  // Convert ext groups: https://github.com/micromatch/picomatch#advanced-globbing
  pattern = EXT_GROUPS
    .replace_all(&pattern, |caps: &Captures| {
      format!(
        "<{}:{}>",
        caps.get(2).unwrap().as_str(),
        match caps.get(1).unwrap().as_str() {
          "!" => "0,0", // Not allowed, error below!
          "@" => "1,1",
          "+" => "1,",
          "?" => "0,1",
          _ => "0,",
        }
      )
    })
    .to_string();

  // This is not supported in wax: https://github.com/olson-sean-k/wax/issues/55
  if pattern.contains(":0,0") {
    return Err(BuildError::invalid_glob_custom(
      base_pattern.to_owned(),
      "the `!(pattern)` extglob group is not supported".into(),
    ));
  }

  // Convert posix: https://github.com/micromatch/picomatch#posix-brackets
  for (find, replace) in POSIX_EXPRS.iter() {
    if pattern.contains(find) {
      pattern = pattern.replace(find, replace);
    }
  }

  Ok(pattern)
}

pub fn create_glob(pattern: &str) -> Result<Glob<'static>, BuildError> {
  let pattern = convert_js_to_rust(pattern)?;

  Glob::new(&pattern)
    .map(Glob::into_owned)
    .map_err(|error| BuildError::invalid_glob(pattern, error))
}

pub fn create_glob_with_star_prefix(pattern: &str) -> Result<Glob<'static>, BuildError> {
  let pattern =
    if pattern.starts_with("**/") { pattern.to_owned() } else { format!("**/{pattern}") };

  create_glob(&pattern)
}

pub fn create_globset(globs: Vec<Glob>) -> Result<Any, BuildError> {
  wax::any(globs).map_err(BuildError::invalid_globset)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn converts_groups() {
    assert_eq!(convert_js_to_rust("a*(z)").unwrap(), "a<z:0,>");
    assert_eq!(convert_js_to_rust("a+(z)").unwrap(), "a<z:1,>");
    assert_eq!(convert_js_to_rust("a?(z)").unwrap(), "a<z:0,1>");
    assert_eq!(convert_js_to_rust("a@(z)").unwrap(), "a<z:1,1>");

    // multiple
    assert_eq!(convert_js_to_rust("?(foo).?(bar)").unwrap(), "<foo:0,1>.<bar:0,1>");
  }

  #[test]
  #[should_panic(expected = "the `!(pattern)` extglob group is not supported")]
  fn errors_for_negated_group() {
    convert_js_to_rust("a!(z)").unwrap();
  }

  #[test]
  fn converts_posix() {
    assert_eq!(convert_js_to_rust("*/[:alpha:]").unwrap(), "*/[a-zA-Z]");
    assert_eq!(convert_js_to_rust("**/[:space:]+").unwrap(), "**/[ \\t\\r\\n\\v\\f]+");
  }
}
