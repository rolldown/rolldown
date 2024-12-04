use std::{fmt::Debug, sync::Arc};

use rolldown_sourcemap::{Source, SourceJoiner};

#[derive(Clone, Default)]
pub struct RenderedModule {
  inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
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
    self.inner_code.as_ref().map(|sources| {
      let mut joiner = SourceJoiner::default();

      for source in sources.iter() {
        joiner.append_source(source);
      }

      joiner.join().0
    })
  }

  pub fn iter_source(&self) -> impl Iterator<Item = &Box<dyn Source + Send + Sync>> {
    if let Some(code) = &self.inner_code {
      code.as_ref().iter()
    } else {
      [].iter()
    }
  }
}
