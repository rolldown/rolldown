use std::{fmt::Debug, sync::Arc};

use rolldown_sourcemap::Source;

#[derive(Clone)]
pub struct RenderedModule {
  pub inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl Debug for RenderedModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RenderedModule").finish()
  }
}

impl RenderedModule {
  pub fn new(sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>) -> Self {
    Self { inner_code: sources }
  }

  pub fn code(&self) -> Option<String> {
    self
      .inner_code
      .as_ref()
      .and_then(|sources| sources.get(1).map(|source| source.content().to_string()))
  }
}
