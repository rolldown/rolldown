use derive_more::Debug;
use std::sync::Arc;
use std::{future::Future, pin::Pin};

/// Rewrites a whole batch of sources in one call.
///
/// A JS callback implements this function, and every call crosses the napi boundary. The returned
/// `Vec` must match the source list by index. The caller rejects any other length.
type SourceMapPathTransformFn = dyn Fn(
    /* sources */ Vec<String>,
    /* sourcemap path */ &str,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
#[debug("SourceMapPathTransform::Fn(...)")]
pub struct SourceMapPathTransform(Arc<SourceMapPathTransformFn>);

impl SourceMapPathTransform {
  pub fn new(f: Arc<SourceMapPathTransformFn>) -> Self {
    Self(f)
  }

  pub async fn call(
    &self,
    sources: Vec<String>,
    sourcemap_path: &str,
  ) -> anyhow::Result<Vec<String>> {
    self.0(sources, sourcemap_path).await
  }
}
