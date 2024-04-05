use futures::Future;
use rolldown_common::RenderedChunk;
use rolldown_error::BuildError;
use std::fmt::Debug;
use std::pin::Pin;

pub type AddonFunction = dyn Fn(
    RenderedChunk,
  ) -> Pin<Box<(dyn Future<Output = Result<Option<String>, BuildError>> + Send + 'static)>>
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

impl Default for AddonOutputOption {
  fn default() -> Self {
    Self::String(None)
  }
}

impl AddonOutputOption {
  pub async fn call(&self, chunk: RenderedChunk) -> Result<Option<String>, BuildError> {
    match self {
      Self::String(value) => Ok(value.clone()),
      Self::Fn(value) => value(chunk).await,
    }
  }
}
