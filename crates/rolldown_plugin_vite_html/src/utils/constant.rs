use std::{
  pin::Pin,
  sync::{Arc, LazyLock},
};

use regex::Regex;
use rolldown_common::{Output, OutputChunk};

pub const MODULE_PRELOAD_POLYFILL: &str = "vite/modulepreload-polyfill";

pub static INLINE_IMPORT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"\bimport\s*\(("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\)"#).unwrap());

pub static IMPORT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"\bimport\s*(?:"[^"]*[^\\]"|'[^']*[^\\]');*"#).unwrap());

pub static COMMENT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?m)/\*[\s\S]*?\*/|//.*$").unwrap());

pub type TransformIndexHtml = dyn Fn(
    &str,
    &str,
    &str,
    Option<Vec<Output>>,
    Option<Arc<OutputChunk>>,
    &'static str,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>>
  + Send
  + Sync;

pub type SetModuleSideEffects =
  dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync;
