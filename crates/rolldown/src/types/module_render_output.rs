use std::sync::Arc;

use rolldown_common::RenderedModule;
use rolldown_sourcemap::SourceMap;

pub struct ModuleRenderOutput<'a> {
  pub module_path: &'a str,
  pub module_pretty_path: &'a str,
  pub rendered_module: RenderedModule,
  pub rendered_content: String,
  pub sourcemap: Option<Arc<SourceMap>>,
}
