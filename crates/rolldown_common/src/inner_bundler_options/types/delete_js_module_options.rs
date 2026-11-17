use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type DeleteJsModuleOptionsFn = dyn Fn(Vec<String>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
  + Send
  + Sync;

/// Deletes the module options (`moduleSideEffects`, `meta`) that the JS plugin layer holds for the
/// given module ids. See `internal-docs/cache/implementation.md`.
#[derive(Clone, Debug)]
#[debug("DeleteJsModuleOptionsFn::Fn(...)")]
pub struct DeleteJsModuleOptions(Arc<DeleteJsModuleOptionsFn>);

impl DeleteJsModuleOptions {
  pub fn new(f: Arc<DeleteJsModuleOptionsFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, module_ids: Vec<String>) -> anyhow::Result<()> {
    self.0(module_ids).await
  }
}
