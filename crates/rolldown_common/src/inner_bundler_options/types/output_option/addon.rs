use derive_more::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::RollupRenderedChunk;

pub type AddonFunction = dyn Fn(
    Arc<RollupRenderedChunk>,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum AddonOutputOption {
  #[debug("AddonFunction::String({})", "{0:?}")]
  String(Option<String>),
  #[debug("AddonFunction::Fn(...)")]
  Fn(Arc<AddonFunction>),
}

impl AddonOutputOption {
  pub async fn call(&self, chunk: Arc<RollupRenderedChunk>) -> anyhow::Result<Option<String>> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => value(chunk).await,
    }
  }
}
