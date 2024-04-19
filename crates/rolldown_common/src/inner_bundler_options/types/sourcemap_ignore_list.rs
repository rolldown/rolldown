use std::fmt::Debug;
use std::{future::Future, pin::Pin};

pub type SourceMapIgnoreListFn = dyn Fn(&str, &str) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
  + Send
  + Sync;

pub struct SourceMapIgnoreList(Box<SourceMapIgnoreListFn>);

impl Debug for SourceMapIgnoreList {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SourceMapIgnoreList::Fn(...)")
  }
}

impl SourceMapIgnoreList {
  pub fn new(f: Box<SourceMapIgnoreListFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, source: &str, sourcemap_path: &str) -> anyhow::Result<bool> {
    self.0(source, sourcemap_path).await
  }
}
