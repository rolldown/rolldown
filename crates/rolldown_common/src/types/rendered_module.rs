use std::sync::Arc;

use derive_more::derive::Debug;
use rolldown_rstr::Rstr;
use rolldown_sourcemap::{Source, SourceJoiner};

#[derive(Clone, Default, Debug)]
#[debug("RenderedModule")]
pub struct RenderedModule {
  inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  pub rendered_exports: Vec<Rstr>,
}

impl RenderedModule {
  pub fn new(
    sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
    rendered_exports: Vec<Rstr>,
  ) -> Self {
    Self { inner_code: sources, rendered_exports }
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
