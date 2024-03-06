use rolldown_common::NormalModule;
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};
use rolldown_sourcemap::SourceMap;
use string_wizard::MagicString;

use crate::types::module_render_context::ModuleRenderContext;

pub struct RenderedNormalModuleOutput {
  pub code: MagicString<'static>,
  pub map: Option<SourceMap>,
}

#[allow(clippy::unnecessary_wraps)]
pub fn render_normal_module(
  module: &NormalModule,
  _ctx: &ModuleRenderContext<'_>,
  ast: &OxcProgram,
) -> Option<RenderedNormalModuleOutput> {
  if ast.program().body.is_empty() {
    None
  } else {
    let generated_code = OxcCompiler::print(ast);
    let mut source = MagicString::new(generated_code);

    source.prepend(format!("// {}\n", module.pretty_path));

    Some(RenderedNormalModuleOutput { code: source, map: None })
  }
}
