use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type ResetJsModuleOptionsFn = dyn Fn(Option<Vec<String>>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
  + Send
  + Sync;

/// Resets the module options (`moduleSideEffects`, `meta`) that the JS plugin layer keeps, for all
/// modules (`None`) or for the given module ids. See `internal-docs/cache/implementation.md`.
#[derive(Clone, Debug)]
#[debug("ResetJsModuleOptionsFn::Fn(...)")]
pub struct ResetJsModuleOptions(Arc<ResetJsModuleOptionsFn>);

impl ResetJsModuleOptions {
  pub fn new(f: Arc<ResetJsModuleOptionsFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, module_ids: Option<Vec<String>>) -> anyhow::Result<()> {
    self.0(module_ids).await
  }
}
