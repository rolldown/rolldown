use std::sync::Arc;

use derive_more::derive::Debug;
use oxc::span::CompactStr;
use rolldown_sourcemap::{Source, SourceJoiner};

#[derive(Clone, Default, Debug)]
#[debug("RenderedModule")]
pub struct RenderedModule {
  inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  pub rendered_exports: Vec<CompactStr>,
  pub exec_order: u32,
}

impl RenderedModule {
  pub fn new(
    sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
    rendered_exports: Vec<CompactStr>,
    exec_order: u32,
  ) -> Self {
    Self { inner_code: sources, rendered_exports, exec_order }
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
