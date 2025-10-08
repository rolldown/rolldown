use std::sync::LazyLock;

use regex::Regex;

pub static INLINE_IMPORT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"\bimport\s*\(("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\)"#).unwrap());
