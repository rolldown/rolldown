use derive_more::Debug;
use rolldown_error::SingleBuildResult;
use std::sync::Arc;
use std::{future::Future, pin::Pin};

type SourceMapPathTransformFn = dyn Fn(&str, &str) -> Pin<Box<dyn Future<Output = SingleBuildResult<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
#[debug("SourceMapPathTransform::Fn(...)")]
pub struct SourceMapPathTransform(Arc<SourceMapPathTransformFn>);

impl SourceMapPathTransform {
  pub fn new(f: Arc<SourceMapPathTransformFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, source: &str, sourcemap_path: &str) -> SingleBuildResult<String> {
    self.0(source, sourcemap_path).await
  }
}
