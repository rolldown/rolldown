use oxc::ast::ast::{Declaration, Statement};
use rolldown_common::{ConcatenateWrappedModuleKind, WrapKind};
use rolldown_utils::{index_vec_ext::IndexVecRefExt, rayon::ParallelIterator as _};

use crate::type_alias::IndexEcmaAst;

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Pre-finalization pass: flag wrapped (`WrapKind::Esm`) modules whose `__esm` closure body is
  /// empty, i.e. every top-level statement is a hoisted function declaration (lifted out of the
  /// closure) or a source-less export clause, so nothing remains inside the closure.
  ///
  /// Calling such a module's `init_*()` is a no-op, so [`super::super::module_finalizers`] marks
  /// those call sites `@__PURE__` and the default `dce-only` minifier drops them (and, once the
  /// wrapper is unreferenced, the wrapper declaration / runtime helper too).
  ///
  /// Must run before [`Self::finalize_modules`], which finalizes modules in parallel: an
  /// importer needs the importee's flag while emitting the init call, so it cannot be computed
  /// during finalization itself.
  pub(super) fn compute_init_is_noop(&mut self, ast_table: &IndexEcmaAst) {
    // Classify in parallel (read-only); the cheap flag write-back stays sequential.
    let keep_names = self.options.keep_names;
    let metas = &self.link_output.metas;
    let noop_modules = ast_table
      .par_iter_enumerated()
      .filter_map(|(idx, ast)| {
        let ast = ast.as_ref()?;
        let meta = &metas[idx];
        if !meta.is_included || !matches!(meta.wrap_kind(), WrapKind::Esm) {
          return None;
        }
        // Restrict to standalone wrappers. In a concatenated group the shared `init_*` runs the
        // whole group's closure, so this module's own empty body wouldn't prove the call is a
        // no-op (a sibling could carry content).
        if !matches!(meta.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None) {
          return None;
        }
        // Shimmed missing exports emit `<name> = void 0;` assignments *into* the closure
        // (generated after this pass, so they aren't visible in the AST below). Their presence
        // makes the init non-empty.
        if !meta.shimmed_missing_exports.is_empty() {
          return None;
        }
        // Require *every* top-level statement to be a hoisted function declaration. Such a
        // module has nothing to put inside its `__esm` closure: function declarations are lifted
        // out, and the absence of imports / re-exports / side-effecting statements means there is
        // no init-call glue or eager code to run — under plain tree-shaking *or*
        // `strictExecutionOrder` (which can force init calls from re-export statements even when
        // their binding is unused). We deliberately check non-included statements too: a
        // statement that is *not* a function declaration is treated as making the init non-empty,
        // which only ever keeps a redundant (harmless) init call — never drops a needed one.
        let is_noop =
          ast.program().body.iter().all(|stmt| contributes_no_closure_body(stmt, keep_names));
        is_noop.then_some(idx)
      })
      .collect::<Vec<_>>();

    for idx in noop_modules {
      self.link_output.metas[idx].init_is_noop = true;
    }
  }
}

/// Whether a top-level statement contributes nothing to the `__esm` closure body. Qualifying
/// statements:
/// - function declarations (`function f(){}`) — hoisted out of the closure;
/// - `export function f(){}` — same, just re-exported;
/// - source-less export clauses (`export {}`, `export { a, b }`) — namespace-level only; any
///   actual bindings they reference live in separate statements that are checked on their own.
///
/// Everything else (variables, classes, expressions, and crucially any `import`/`export … from`
/// which can lower to an eager init call inside the closure) is treated as making the init
/// non-empty. Being conservative here only keeps a redundant (harmless) init call — it never
/// drops a needed one. A [`debug_assert!`] in the finalizer guards this classification against
/// the actual closure contents.
fn contributes_no_closure_body(stmt: &Statement, keep_names: bool) -> bool {
  match stmt {
    // With `keepNames`, a function declaration gets a `__name(fn, "...")` assignment inserted
    // into the wrapper closure to preserve `fn.name` (see `insert_keep_name_statements`), so the
    // init is no longer a no-op.
    Statement::FunctionDeclaration(_) => !keep_names,
    Statement::ExportNamedDeclaration(export) => {
      export.source.is_none()
        && match &export.declaration {
          None => true,
          Some(Declaration::FunctionDeclaration(_)) => !keep_names,
          Some(_) => false,
        }
    }
    _ => false,
  }
}
