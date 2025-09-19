use std::{future::Future, pin::Pin, sync::Arc};

use derive_more::Debug;
use rolldown_utils::pattern_filter::StringOrRegex;

type IsExternalFn = dyn Fn(
    &str,         // specifier
    Option<&str>, // importer
    bool,         // is_resolved
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Debug, Clone)]
pub enum IsExternal {
  #[debug("IsExternal::Fn(..)")]
  Fn(Option<Arc<IsExternalFn>>),
  #[debug("IsExternal::StringOrRegex({})", "{0:?}")]
  StringOrRegex(Vec<StringOrRegex>),
}

impl Default for IsExternal {
  fn default() -> Self {
    IsExternal::Fn(None)
  }
}

impl From<Vec<String>> for IsExternal {
  fn from(value: Vec<String>) -> Self {
    IsExternal::StringOrRegex(value.into_iter().map(StringOrRegex::String).collect())
  }
}

impl IsExternal {
  pub async fn call(
    &self,
    specifier: &str,
    importer: Option<&str>,
    is_resolved: bool,
  ) -> anyhow::Result<bool> {
    Ok(match self {
      IsExternal::StringOrRegex(patterns) => patterns.iter().any(|p| match p {
        StringOrRegex::String(s) => s == specifier,
        StringOrRegex::Regex(r) => r.matches(specifier),
      }),
      IsExternal::Fn(Some(is_external)) => {
        return is_external(specifier, importer, is_resolved).await;
      }
      IsExternal::Fn(None) => false,
    })
  }
}
