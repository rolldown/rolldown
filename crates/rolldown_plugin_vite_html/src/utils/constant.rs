use std::sync::LazyLock;

use regex::Regex;

pub const MODULE_PRELOAD_POLYFILL: &str = "vite/modulepreload-polyfill";

pub static INLINE_IMPORT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"\bimport\s*\(("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\)"#).unwrap());

pub static IMPORT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"\bimport\s*(?:"[^"]*[^\\]"|'[^']*[^\\]');*"#).unwrap());

pub static COMMENT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?m)/\*[\s\S]*?\*/|//.*$").unwrap());
