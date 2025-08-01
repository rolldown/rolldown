use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type MarkModuleLoadedFn = dyn Fn(&str, bool) -> Pin<Box<(dyn Future<Output = anyhow::Result<()>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
#[debug("MarkModuleLoadedFn::Fn(...)")]
// Shared async callback for tracking module loading status
pub struct MarkModuleLoaded(Arc<MarkModuleLoadedFn>);

impl MarkModuleLoaded {
  pub fn new(f: Arc<MarkModuleLoadedFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, module_id: &str, success: bool) -> anyhow::Result<()> {
    self.0(module_id, success).await
  }
}
