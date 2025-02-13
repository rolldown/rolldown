use std::sync::Arc;

use derive_more::derive::Debug;
use rolldown_sourcemap::{Source, SourceJoiner};

#[derive(Clone, Default, Debug)]
#[debug("RenderedModule")]
pub struct RenderedModule {
  inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl RenderedModule {
  pub fn new(sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>) -> Self {
    Self { inner_code: sources }
  }

  pub fn code(&self) -> Option<String> {
    self.inner_code.as_ref().map(|sources| {
      let mut joiner = SourceJoiner::default();

      for source in sources.iter() {
        joiner.append_source(source);
      }

      joiner.join().0
    })
  }
}
