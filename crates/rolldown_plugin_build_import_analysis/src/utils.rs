use regex::Regex;

pub static DYNAMIC_IMPORT_RE: std::sync::LazyLock<Regex> =
  std::sync::LazyLock::new(|| Regex::new(r#"\bimport\s*\(\s*['\"`]"#).unwrap());
