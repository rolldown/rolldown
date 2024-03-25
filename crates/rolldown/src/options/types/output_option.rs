use futures::Future;
use rolldown_common::RenderedChunk;
use rolldown_error::BuildError;
use std::fmt::Debug;
use std::pin::Pin;

// pub type AddonFn = dyn Fn(RenderedChunk) -> String + Sync + Send;

pub type AddonFn = dyn Fn(RenderedChunk) -> Pin<Box<(dyn Future<Output = Option<String>> + Send + 'static)>>
  + Send
  + Sync;

pub enum Addon {
  String(Option<String>),
  Fn(Box<AddonFn>),
}

impl Debug for Addon {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::String(value) => write!(f, "Addon::String({value:?})"),
      Self::Fn(_) => write!(f, "Addon::Fn(...)"),
    }
  }
}

impl Default for Addon {
  fn default() -> Self {
    Self::String(None)
  }
}

impl Addon {
  pub async fn call(&self, chunk: RenderedChunk) -> Result<Option<String>, BuildError> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => Ok(value(chunk).await),
    }
  }
}
