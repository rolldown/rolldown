use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type ResetJsModuleOptionsFn = dyn Fn(Option<Vec<String>>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
  + Send
  + Sync;

/// Resets the module options kept on the JS side, for all modules (`None`) or the given ones.
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
