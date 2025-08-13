pub mod apply_inner_plugins;
pub mod augment_chunk_hash;
pub mod chunk;
pub mod ecma_visitors;
pub mod load_entry_module;
pub mod load_source;
pub mod normalize_options;
pub mod parse_to_ecma_ast;
pub mod pre_process_ecma_ast;
pub mod process_code_and_sourcemap;
pub mod renamer;
pub mod render_chunks;
pub mod render_ecma_module;
pub mod resolve_id;
pub mod text_to_esm;
pub mod transform_source;
pub mod tweak_ast_for_scanning;
pub mod uuid;

use oxc::{allocator::TakeIn, ast_visit::VisitMut};
use rolldown_common::AstScopes;
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashSet;

use super::module_finalizers::{ScopeHoistingFinalizer, ScopeHoistingFinalizerContext};

#[tracing::instrument(level = "trace", skip_all)]
pub fn finalize_normal_module<'me>(
  ctx: ScopeHoistingFinalizerContext<'me>,
  ast: &'me mut EcmaAst,
  ast_scope: &'me AstScopes,
) -> ScopeHoistingFinalizerContext<'me> {
  ast.program.with_mut(|fields| {
    let (oxc_program, alloc) = (fields.program, fields.allocator);
    let mut finalizer = ScopeHoistingFinalizer {
      alloc,
      ctx,
      scope: ast_scope,
      snippet: AstSnippet::new(alloc),
      comments: oxc_program.comments.take_in(alloc),
      generated_init_esm_importee_ids: FxHashSet::default(),
      scope_stack: vec![],
      top_level_var_bindings: FxIndexSet::default(),
      is_top_level: false,
    };
    finalizer.visit_program(oxc_program);
    oxc_program.comments = finalizer.comments.take_in(alloc);
    finalizer.ctx
  })
}
