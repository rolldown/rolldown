use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;
use rolldown_utils::pattern_filter::StringOrRegex;

/// Decides a whole batch of sources in one call.
///
/// A JS callback implements this function, and every call crosses the napi boundary. The returned
/// `Vec` must match the source list by index. The caller rejects any other length.
pub type SourceMapIgnoreListFn = dyn Fn(
    /* sources */ Vec<String>,
    /* sourcemap path */ &str,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<bool>>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum SourceMapIgnoreList {
  Boolean(bool),
  StringOrRegex(StringOrRegex),
  #[debug("SourceMapIgnoreList::Fn(...)")]
  Fn(Arc<SourceMapIgnoreListFn>),
}

impl SourceMapIgnoreList {
  pub fn new(f: Arc<SourceMapIgnoreListFn>) -> Self {
    Self::Fn(f)
  }

  pub fn from_bool(value: bool) -> Self {
    Self::Boolean(value)
  }

  pub fn from_string_or_regex(pattern: StringOrRegex) -> Self {
    Self::StringOrRegex(pattern)
  }

  pub fn exec_static(&self, source: &str) -> bool {
    match self {
      Self::Boolean(value) => *value,
      Self::StringOrRegex(pattern) => match pattern {
        StringOrRegex::String(s) => source.contains(s),
        StringOrRegex::Regex(r) => r.matches(source),
      },
      Self::Fn(_) => {
        unreachable!("exec_static should only be called for `Boolean` or `StringOrRegex` variants")
      }
    }
  }

  pub async fn exec_dynamic(
    &self,
    sources: Vec<String>,
    sourcemap_path: &str,
  ) -> anyhow::Result<Vec<bool>> {
    match self {
      Self::Boolean(_) | Self::StringOrRegex(_) => {
        unreachable!("exec_dynamic should only be called for Fn variant")
      }
      Self::Fn(f) => f(sources, sourcemap_path).await,
    }
  }
}
