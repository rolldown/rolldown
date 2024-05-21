use oxc::ast::VisitMut;
use rolldown_common::NormalModule;
use rolldown_oxc_utils::{AstSnippet, OxcAst};

use super::finalizer::{Finalizer, FinalizerContext};

pub mod chunk;
pub mod extract_hash_pattern;
pub mod hash_placeholder;
pub mod load_source;
pub mod normalize_options;
pub mod parse_to_ast;
pub mod renamer;
pub mod render_chunks;
pub mod render_normal_module;
pub mod reserved_names;
pub mod resolve_id;
pub mod transform_source;

pub(crate) fn is_in_rust_test_mode() -> bool {
  static TEST_MODE: once_cell::sync::Lazy<bool> =
    once_cell::sync::Lazy::new(|| std::env::var("ROLLDOWN_TEST").is_ok());
  *TEST_MODE
}

#[tracing::instrument(level = "trace", skip_all)]
pub fn finalize_normal_module(module: &NormalModule, ctx: FinalizerContext<'_>, ast: &mut OxcAst) {
  ast.with_mut(|fields| {
    let (oxc_program, alloc) = (fields.program, fields.allocator);
    let mut finalizer =
      Finalizer { alloc, ctx, scope: &module.scope, snippet: AstSnippet::new(alloc) };
    finalizer.visit_program(oxc_program);
  });
}
