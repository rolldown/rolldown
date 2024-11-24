use rolldown_common::{Output, SharedNormalizedBundlerOptions};

#[derive(Debug)]
pub struct HookGenerateBundleArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
  pub bundle: &'a mut Vec<Output>,
  pub is_write: bool,
}
