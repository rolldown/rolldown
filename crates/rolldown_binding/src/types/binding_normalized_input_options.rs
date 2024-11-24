use napi_derive::napi;
use rolldown::SharedNormalizedBundlerOptions;

#[napi]
pub struct BindingNormalizedInputOptions {
  inner: SharedNormalizedBundlerOptions,
}

#[napi]
impl BindingNormalizedInputOptions {
  pub fn new(inner: SharedNormalizedBundlerOptions) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn shim_missing_exports(&self) -> bool {
    self.inner.shim_missing_exports
  }
}
