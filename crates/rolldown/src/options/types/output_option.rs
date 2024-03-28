use futures::Future;
use rolldown_common::RenderedChunk;
use rolldown_error::BuildError;
use std::fmt::Debug;
use std::pin::Pin;

pub type BannerFn = dyn Fn(RenderedChunk) -> Pin<Box<(dyn Future<Output = Option<String>> + Send + 'static)>>
  + Send
  + Sync;

pub enum Banner {
  String(Option<String>),
  Fn(Box<BannerFn>),
}

impl Debug for Banner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::String(value) => write!(f, "Banner::String({value:?})"),
      Self::Fn(_) => write!(f, "Banner::Fn(...)"),
    }
  }
}

impl Default for Banner {
  fn default() -> Self {
    Self::String(None)
  }
}

impl Banner {
  pub async fn call(&self, chunk: RenderedChunk) -> Result<Option<String>, BuildError> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => Ok(value(chunk).await),
    }
  }
}
