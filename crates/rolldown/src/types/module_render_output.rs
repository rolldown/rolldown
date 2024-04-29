use rolldown_common::{FilePath, RenderedModule};
use rolldown_sourcemap::SourceMap;

pub struct ModuleRenderOutput<'a> {
  pub module_path: FilePath,
  pub module_pretty_path: &'a str,
  pub rendered_module: RenderedModule,
  pub rendered_content: String,
  pub sourcemap: Option<SourceMap>,
  pub lines_count: u32,
}
