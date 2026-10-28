use std::sync::Arc;

use derive_more::derive::Debug;
use oxc_str::CompactStr;
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

  /// Length of the rendered code in UTF-8 bytes, the unit of the package size report.
  /// `rendered_utf16_length` is the unit JavaScript sees as `code.length`.
  pub fn rendered_byte_length(&self) -> usize {
    self.inner_code.as_ref().map_or(0, |sources| {
      sources.iter().map(|source| source.content().len()).sum::<usize>()
        + sources.len().saturating_sub(1)
    })
  }

  /// Length of the rendered code in UTF-16 code units, which is what JavaScript's `String.prototype.length`
  /// returns for `code`. Computed from the source pieces without joining or copying them.
  pub fn rendered_utf16_length(&self) -> usize {
    self.inner_code.as_ref().map_or(0, |sources| {
      sources.iter().map(|source| simd_utf16_len::utf16_len(source.content())).sum::<usize>()
        + sources.len().saturating_sub(1)
    })
  }
}
