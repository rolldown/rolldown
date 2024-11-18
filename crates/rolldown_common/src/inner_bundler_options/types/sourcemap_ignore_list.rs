use std::fmt::Debug;
use std::sync::Arc;
use std::{future::Future, pin::Pin};

pub type SourceMapIgnoreListFn = dyn Fn(&str, &str) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone)]
pub struct SourceMapIgnoreList(Arc<SourceMapIgnoreListFn>);

impl Debug for SourceMapIgnoreList {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SourceMapIgnoreList::Fn(...)")
  }
}

impl SourceMapIgnoreList {
  pub fn new(f: Arc<SourceMapIgnoreListFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, source: &str, sourcemap_path: &str) -> anyhow::Result<bool> {
    self.0(source, sourcemap_path).await
  }
}
