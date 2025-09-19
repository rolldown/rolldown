use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use crate::RollupPreRenderedChunk;

type ChunkFilenamesFunction = dyn Fn(
    &RollupPreRenderedChunk,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum ChunkFilenamesOutputOption {
  #[debug("ChunkFilenamesOutputOption::String({_0:?})")]
  String(String),
  #[debug("ChunkFilenamesOutputOption::Fn(...)")]
  Fn(Arc<ChunkFilenamesFunction>),
}

impl ChunkFilenamesOutputOption {
  pub async fn call(&self, chunk: &RollupPreRenderedChunk) -> anyhow::Result<String> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => value(chunk).await,
    }
  }
}

impl From<String> for ChunkFilenamesOutputOption {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}
