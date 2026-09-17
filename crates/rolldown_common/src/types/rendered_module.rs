use std::sync::Arc;

use derive_more::derive::Debug;
use oxc_str::CompactStr;
use rolldown_sourcemap::{Source, SourceJoiner};

#[derive(Clone, Default, Debug)]
#[debug("RenderedModule")]
pub struct RenderedModule {
  inner_code: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  deferred_runtime_imports: Option<String>,
  pub rendered_exports: Vec<CompactStr>,
  pub exec_order: u32,
}

impl RenderedModule {
  pub fn new(
    sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
    deferred_runtime_imports: Option<String>,
    rendered_exports: Vec<CompactStr>,
    exec_order: u32,
  ) -> Self {
    Self { inner_code: sources, deferred_runtime_imports, rendered_exports, exec_order }
  }

  pub fn code(&self) -> Option<String> {
    if self.inner_code.is_none() && self.deferred_runtime_imports.is_none() {
      return None;
    }
    let mut joiner = SourceJoiner::default();
    if let Some(sources) = &self.inner_code {
      for source in sources.iter() {
        joiner.append_source(source);
      }
    }
    if let Some(deferred) = &self.deferred_runtime_imports {
      joiner.append_source(deferred.as_str());
    }

    Some(joiner.join().0)
  }

  pub fn rendered_length(&self) -> usize {
    let sources = self.inner_code.as_deref().unwrap_or_default();
    let source_count = sources.len() + usize::from(self.deferred_runtime_imports.is_some());
    sources.iter().map(|source| source.content().len()).sum::<usize>()
      + self.deferred_runtime_imports.as_ref().map_or(0, String::len)
      + source_count.saturating_sub(1)
  }
}
