use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use crate::RollupRenderedChunk;

pub type AddonFunction = dyn Fn(
    &RollupRenderedChunk,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'static)>>
  + Send
  + Sync;

pub enum AddonOutputOption {
  String(Option<String>),
  Fn(Box<AddonFunction>),
}

impl Debug for AddonOutputOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::String(value) => write!(f, "AddonFunction::String({value:?})"),
      Self::Fn(_) => write!(f, "AddonFunction::Fn(...)"),
    }
  }
}

impl AddonOutputOption {
  pub async fn call(&self, chunk: &RollupRenderedChunk) -> anyhow::Result<Option<String>> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => value(chunk).await,
    }
  }
}
