use oxc::ast::VisitMut;
use rolldown_common::NormalModule;
use rolldown_oxc_utils::{AstSnippet, OxcAst};

use super::module_finalizers::scope_hoisting::{
  ScopeHoistingFinalizer, ScopeHoistingFinalizerContext,
};
pub mod augment_chunk_hash;
pub mod call_expression_ext;
pub mod chunk;
pub mod extract_hash_pattern;
pub mod extract_meaningful_input_name_from_path;
pub mod fold_const_value;
pub mod hash_placeholder;
pub mod load_source;
pub mod make_ast_symbol_and_scope;
pub mod normalize_options;
pub mod parse_to_ast;
pub mod pre_process_ast;
pub mod renamer;
pub mod render_chunks;
pub mod render_normal_module;
pub mod resolve_id;
pub mod transform_source;
pub mod tweak_ast_for_scanning;

#[tracing::instrument(level = "trace", skip_all)]
pub fn finalize_normal_module(
  module: &NormalModule,
  ctx: ScopeHoistingFinalizerContext<'_>,
  ast: &mut OxcAst,
) {
  ast.program.with_mut(|fields| {
    let (oxc_program, alloc) = (fields.program, fields.allocator);
    let mut finalizer =
      ScopeHoistingFinalizer { alloc, ctx, scope: &module.scope, snippet: AstSnippet::new(alloc) };
    finalizer.visit_program(oxc_program);
  });
}
