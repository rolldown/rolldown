use rolldown_common::Output;

#[derive(Debug)]
pub struct HookGenerateBundleArgs<'a> {
  pub bundle: &'a mut Vec<Output>,
  pub is_write: bool,
}
