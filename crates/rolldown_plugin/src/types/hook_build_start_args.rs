use rolldown_common::SharedNormalizedBundlerOptions;

#[derive(Debug)]
pub struct HookBuildStartArgs<'a> {
  pub options: &'a SharedNormalizedBundlerOptions,
}
