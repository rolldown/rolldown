use std::sync::Arc;
use std::{future::Future, pin::Pin};

use derive_more::Debug;

pub type CreateJsModuleOptionsFn = dyn Fn(
    String,
    Option<u32>,
    Option<String>,
    Option<String>,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>
  + Send
  + Sync;

/// Creates the module options kept on the JS side for a new module, from the JS resolution with
/// the given receipt (`ResolvedId::module_options_receipt`).
///
/// With the persistent cache, two more values come from saved results, each as the text of
/// `JsModuleMeta::to_entries_json`: the `meta` of a reused resolution, which JS holds no receipt
/// for, and the `meta` that the skipped `load` and `transform` hooks of the module returned.
#[derive(Clone, Debug)]
#[debug("CreateJsModuleOptionsFn::Fn(...)")]
pub struct CreateJsModuleOptions(Arc<CreateJsModuleOptionsFn>);

impl CreateJsModuleOptions {
  pub fn new(f: Arc<CreateJsModuleOptionsFn>) -> Self {
    Self(f)
  }

  pub async fn call(
    &self,
    module_id: String,
    receipt: Option<u32>,
    saved_meta: Option<String>,
    saved_own_meta: Option<String>,
  ) -> anyhow::Result<()> {
    self.0(module_id, receipt, saved_meta, saved_own_meta).await
  }
}
