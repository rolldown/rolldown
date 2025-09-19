use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type SourceMapIgnoreListFn = dyn Fn(&str, &str) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
#[debug("SourceMapIgnoreList::Fn(...)")]
pub struct SourceMapIgnoreList(Arc<SourceMapIgnoreListFn>);

impl SourceMapIgnoreList {
  pub fn new(f: Arc<SourceMapIgnoreListFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, source: &str, sourcemap_path: &str) -> anyhow::Result<bool> {
    self.0(source, sourcemap_path).await
  }
}
