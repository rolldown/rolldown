use rolldown_common::{RenderedModule, ResourceId};
use rolldown_sourcemap::SourceMap;

pub struct ModuleRenderOutput<'a> {
  pub module_path: ResourceId,
  pub module_pretty_path: &'a str,
  pub rendered_module: RenderedModule,
  pub rendered_content: String,
  pub sourcemap: Option<SourceMap>,
  pub lines_count: u32,
}
