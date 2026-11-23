use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type CreateJsModuleOptionsFn = dyn Fn(String, Option<u32>) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
  + Send
  + Sync;

/// Creates the module options kept on the JS side for a new module, from the JS resolution with
/// the given receipt (`ResolvedId::module_options_receipt`).
#[derive(Clone, Debug)]
#[debug("CreateJsModuleOptionsFn::Fn(...)")]
pub struct CreateJsModuleOptions(Arc<CreateJsModuleOptionsFn>);

impl CreateJsModuleOptions {
  pub fn new(f: Arc<CreateJsModuleOptionsFn>) -> Self {
    Self(f)
  }

  pub async fn call(&self, module_id: String, receipt: Option<u32>) -> anyhow::Result<()> {
    self.0(module_id, receipt).await
  }
}
