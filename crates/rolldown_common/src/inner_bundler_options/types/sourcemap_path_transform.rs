use std::fmt::Debug;
use std::{future::Future, pin::Pin};

type SourceMapPathTransformFn = dyn Fn(&str, &str) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send + 'static)>>
  + Send
  + Sync;

pub struct SourceMapPathTransform(Box<SourceMapPathTransformFn>);

impl Debug for SourceMapPathTransform {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SourceMapPathTransform::Fn(...)")
  }
}

impl SourceMapPathTransform {
  pub fn new(f: Box<SourceMapPathTransformFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, source: &str, sourcemap_path: &str) -> anyhow::Result<String> {
    self.0(source, sourcemap_path).await
  }
}
