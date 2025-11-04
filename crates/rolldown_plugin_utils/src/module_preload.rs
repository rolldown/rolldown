use std::{pin::Pin, sync::Arc};

pub type ResolveDependenciesFn = dyn Fn(
    &str,
    Vec<String>,
    &str,
    &str,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send>>
  + Send
  + Sync;

#[derive(Debug)]
pub enum ModulePreload {
  False,
  Options(ModulePreloadOptions),
}

impl ModulePreload {
  pub fn is_false(&self) -> bool {
    match self {
      ModulePreload::False => true,
      ModulePreload::Options(_) => false,
    }
  }

  pub fn options(&self) -> Option<&ModulePreloadOptions> {
    match self {
      ModulePreload::False => None,
      ModulePreload::Options(options) => Some(options),
    }
  }
}

#[derive(derive_more::Debug)]
pub struct ModulePreloadOptions {
  /**
   * Whether to inject a module preload polyfill.
   * Note: does not apply to library mode.
   * @default true
   */
  pub polyfill: bool,
  /**
   * Resolve the list of dependencies to preload for a given dynamic import
   * @experimental
   */
  #[debug(skip)]
  pub resolve_dependencies: Option<Arc<ResolveDependenciesFn>>,
}
