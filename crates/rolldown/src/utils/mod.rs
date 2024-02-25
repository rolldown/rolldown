use oxc::ast::VisitMut;
use rolldown_common::NormalModule;
use rolldown_oxc::{AstSnippet, OxcCompiler, OxcProgram};
use string_wizard::MagicString;

use super::{
  finalizer::{Finalizer, FinalizerContext},
  types::module_render_context::ModuleRenderContext,
};

pub mod bitset;
pub mod load_source;
pub mod renamer;
pub mod render_chunks;
pub mod resolve_id;
pub mod transform_source;

pub(crate) fn is_in_rust_test_mode() -> bool {
  static TEST_MODE: once_cell::sync::Lazy<bool> =
    once_cell::sync::Lazy::new(|| std::env::var("ROLLDOWN_TEST").is_ok());
  *TEST_MODE
}

#[allow(clippy::unnecessary_wraps)]
pub fn render_normal_module(
  module: &NormalModule,
  _ctx: &ModuleRenderContext<'_>,
  ast: &OxcProgram,
) -> Option<MagicString<'static>> {
  let generated_code = OxcCompiler::print(ast);
  let mut source = MagicString::new(generated_code);

  source.prepend(format!("// {}\n", module.pretty_path));

  Some(source)
}

pub fn finalize_normal_module(
  module: &NormalModule,
  ctx: FinalizerContext<'_>,
  ast: &mut OxcProgram,
) {
  let (oxc_program, alloc) = ast.program_mut_and_allocator();

  let mut finalizer =
    Finalizer { alloc, ctx, scope: &module.scope, snippet: &AstSnippet::new(alloc) };

  finalizer.visit_program(oxc_program);
}
