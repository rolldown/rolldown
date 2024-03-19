use oxc::codegen::CodegenReturn;
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};

use crate::types::module_render_context::ModuleRenderContext;

pub fn render_normal_module(
  _ctx: &ModuleRenderContext<'_>,
  ast: &OxcProgram,
  enable_sourcemap: Option<String>,
) -> Option<CodegenReturn> {
  if ast.program().body.is_empty() {
    None
  } else {
    Some(OxcCompiler::print(ast, enable_sourcemap))
  }
}
