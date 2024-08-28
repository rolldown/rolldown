use std::{fmt::Debug, future::Future, pin::Pin};

use crate::RollupPreRenderedChunk;

type ChunkFilenamesFunction = dyn Fn(
    &RollupPreRenderedChunk,
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send + 'static)>>
  + Send
  + Sync;

pub enum ChunkFilenamesOutputOption {
  String(String),
  Fn(Box<ChunkFilenamesFunction>),
}

impl Debug for ChunkFilenamesOutputOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::String(value) => write!(f, "ChunkFilenamesOutputOption::String({value:?})"),
      Self::Fn(_) => write!(f, "ChunkFilenamesOutputOption::Fn(...)"),
    }
  }
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
