use rolldown_common::SharedNormalizedBundlerOptions;

#[derive(Debug)]
pub struct HookRenderStartArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
}
