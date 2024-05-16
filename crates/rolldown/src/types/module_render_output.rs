use rolldown_common::{NormalModuleId, RenderedModule};
use rolldown_sourcemap::SourceMap;

pub struct ModuleRenderOutput {
  pub module_id: NormalModuleId,
  pub rendered_module: RenderedModule,
  pub rendered_content: String,
  pub sourcemap: Option<SourceMap>,
  pub lines_count: u32,
}
