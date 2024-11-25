use rolldown_common::{Output, SharedNormalizedBundlerOptions};

#[derive(Debug)]
pub struct HookWriteBundleArgs<'a> {
  pub bundle: &'a mut Vec<Output>,
  pub options: &'a SharedNormalizedBundlerOptions,
}
