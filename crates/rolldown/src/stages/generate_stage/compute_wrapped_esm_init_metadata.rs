use oxc::ast::ast::{Declaration, Statement};
use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ExportsKind, ImportKind, ImportRecordMeta, IndexModules,
  Module, ModuleIdx, NormalModule, PostChunkOptimizationOperation, StmtInfoIdx, StmtInfos,
  WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{index_vec_ext::IndexVecRefExt, rayon::ParallelIterator as _};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::{GenerateStage, order_wrap_state::OrderWrapState};

impl GenerateStage<'_> {
  /// Pre-finalization pass computing, for every included wrapped (`WrapKind::Esm`) module, the
  /// init-call facts the module finalizers consume when emitting `init_*()` calls:
  ///
  /// - [`LinkingMetadata::init_is_noop`]: the module's `__esm` closure body is empty — every
  ///   top-level statement is a hoisted function declaration (lifted out of the closure) or a
  ///   source-less export clause — so call sites can be marked `@__PURE__` and the default
  ///   `dce-only` minifier drops them (and, once the wrapper is unreferenced, the wrapper
  ///   declaration / runtime helper too).
  /// - [`LinkingMetadata::transitive_esm_init_targets`]: per non-included top-level import or
  ///   re-export statement, the wrapped-ESM modules whose `init_*()` calls must be emitted in the
  ///   statement's place. Re-exports keep the legacy forwarding behavior; an order-wrapped ordinary
  ///   import keeps an obligation only when it was live before lowering.
  ///
  /// Must run after chunk assignment (`module_to_chunk`) and final order wrapping, and before
  /// [`Self::finalize_modules`]: modules are finalized in parallel, and an
  /// importer needs its importees' facts while emitting init calls, so neither can be computed
  /// during finalization itself. The transitive targets cannot be computed in the link stage
  /// either: which init calls survive depends on chunk assignment. Cross-chunk wrapper imports are
  /// registered after this pass by `compute_cross_chunk_links`.
  pub(super) fn compute_wrapped_esm_init_metadata(
    &mut self,
    ast_table: &IndexEcmaAst,
    chunk_graph: &ChunkGraph,
    _order_state: &OrderWrapState,
  ) {
    // Classify in parallel (read-only); the cheap write-back stays sequential.
    let keep_names = self.options.keep_names;
    let metas = &self.link_output.metas;
    let modules = &self.link_output.module_table.modules;
    let stmt_infos_vec = &self.link_output.stmt_infos;
    let module_to_chunk = &chunk_graph.module_to_chunk;
    let results = metas
      .par_iter_enumerated()
      .filter_map(|(module_idx, meta)| {
        if !meta.is_included || !matches!(meta.wrap_kind(), WrapKind::Esm) {
          return None;
        }
        let is_noop = init_is_noop(meta, ast_table[module_idx].as_ref(), keep_names);
        let targets_by_stmt = modules[module_idx]
          .as_normal()
          .zip(module_to_chunk[module_idx])
          .filter(|_| module_has_live_chunk(chunk_graph, module_idx))
          .map(|(module, chunk_idx)| {
            transitive_esm_init_targets(
              module,
              meta,
              &stmt_infos_vec[module_idx],
              &EsmInitTargetContext {
                modules,
                metas,
                chunk_graph,
                module_to_chunk,
                chunk_idx,
                order_wrap: meta.hoist_esm_wrapper,
                execution_dependencies: &meta.execution_dependencies,
              },
            )
          })
          .unwrap_or_default();
        (is_noop || !targets_by_stmt.is_empty()).then_some((module_idx, is_noop, targets_by_stmt))
      })
      .collect::<Vec<_>>();

    for (module_idx, is_noop, targets_by_stmt) in results {
      let meta = &mut self.link_output.metas[module_idx];
      meta.init_is_noop = is_noop;
      meta.transitive_esm_init_targets = targets_by_stmt;
    }
  }
}

struct EsmInitTargetContext<'a> {
  modules: &'a IndexModules,
  metas: &'a LinkingMetadataVec,
  chunk_graph: &'a ChunkGraph,
  module_to_chunk: &'a IndexVec<ModuleIdx, Option<ChunkIdx>>,
  chunk_idx: ChunkIdx,
  order_wrap: bool,
  execution_dependencies: &'a rolldown_utils::indexmap::FxIndexSet<ModuleIdx>,
}

/// Whether calling the module's `init_*()` is a no-op because nothing lands inside its `__esm`
/// closure.
fn init_is_noop(meta: &LinkingMetadata, ast: Option<&EcmaAst>, keep_names: bool) -> bool {
  // Restrict to standalone wrappers. In a concatenated group the shared `init_*` runs the
  // whole group's closure, so this module's own empty body wouldn't prove the call is a
  // no-op (a sibling could carry content).
  if !matches!(meta.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None) {
    return false;
  }
  // Shimmed missing exports emit `<name> = void 0;` assignments *into* the closure
  // (generated after this pass, so they aren't visible in the AST below). Their presence
  // makes the init non-empty.
  if !meta.shimmed_missing_exports.is_empty() {
    return false;
  }
  // Require *every* top-level statement to be a hoisted function declaration. Such a
  // module has nothing to put inside its `__esm` closure: function declarations are lifted
  // out, and the absence of imports / re-exports / side-effecting statements means there is
  // no init-call glue or eager code to run — under plain tree-shaking *or*
  // `strictExecutionOrder` (which can force init calls from re-export statements even when
  // their binding is unused). We deliberately check non-included statements too: a
  // statement that is *not* a function declaration is treated as making the init non-empty,
  // which only ever keeps a redundant (harmless) init call — never drops a needed one.
  ast.is_some_and(|ast| {
    ast.program().body.iter().all(|stmt| contributes_no_closure_body(stmt, keep_names))
  })
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

/// For each non-included static import/re-export statement of `module`, the wrapped-ESM modules
/// whose `init_*()` calls the finalizer must emit in the statement's place.
fn transitive_esm_init_targets(
  module: &NormalModule,
  meta: &LinkingMetadata,
  stmt_infos: &StmtInfos,
  ctx: &EsmInitTargetContext<'_>,
) -> FxHashMap<StmtInfoIdx, Vec<ModuleIdx>> {
  // Shared across all excluded re-export statements of this importer, so a barrel subtree is
  // traversed at most once and each target is attributed to the first statement that reaches
  // it (matching the finalizer's per-module emission dedup).
  let mut visited = FxHashSet::default();
  let mut targets_by_stmt = FxHashMap::<StmtInfoIdx, Vec<ModuleIdx>>::default();
  for (stmt_idx, stmt_info) in stmt_infos.iter_enumerated_without_namespace_stmt() {
    if meta.stmt_info_included.has_bit(stmt_idx) {
      continue;
    }
    for &rec_idx in &stmt_info.import_records {
      let rec = &module.import_records[rec_idx];
      if rec.kind != ImportKind::Import {
        continue;
      }
      let is_reexport =
        rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly);
      let Some(root) = rec.resolved_module else { continue };
      let was_live_order_wrap_import = ctx.order_wrap && ctx.execution_dependencies.contains(&root);
      if !is_reexport && !was_live_order_wrap_import {
        continue;
      }
      let mut targets = vec![];
      if ctx.order_wrap {
        collect_order_wrap_esm_init_targets(
          ctx.modules,
          ctx.metas,
          ctx.chunk_graph,
          root,
          &mut visited,
          &mut targets,
        );
      } else {
        collect_legacy_esm_init_targets(
          ctx.modules,
          ctx.metas,
          ctx.module_to_chunk,
          ctx.chunk_idx,
          root,
          &mut visited,
          &mut targets,
        );
      }
      if !targets.is_empty() {
        targets_by_stmt.entry(stmt_idx).or_default().extend(targets);
      }
    }
  }
  targets_by_stmt
}

fn collect_legacy_esm_init_targets(
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
  chunk_idx: ChunkIdx,
  root: ModuleIdx,
  visited: &mut FxHashSet<ModuleIdx>,
  targets: &mut Vec<ModuleIdx>,
) {
  let mut stack = vec![root];
  while let Some(module_idx) = stack.pop() {
    let Module::Normal(importee) = &modules[module_idx] else { continue };
    let importee_linking_info = &metas[importee.idx];
    if !matches!(importee_linking_info.wrap_kind(), WrapKind::Esm) {
      continue;
    }

    if !visited.insert(importee.idx) {
      continue;
    }

    if importee_linking_info.is_included && module_to_chunk[importee.idx] == Some(chunk_idx) {
      targets.push(importee.idx);
    } else {
      for rec in importee.import_records.iter().rev() {
        if let Some(sub_importee_idx) = rec.resolved_module {
          stack.push(sub_importee_idx);
        }
      }
    }
  }
}

/// Find the wrapped-ESM modules an excluded re-export statement must still initialize, by
/// traversing through non-included barrel modules to reach included importees whose wrappers are
/// assigned to a chunk.
fn collect_order_wrap_esm_init_targets(
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  chunk_graph: &ChunkGraph,
  root: ModuleIdx,
  visited: &mut FxHashSet<ModuleIdx>,
  targets: &mut Vec<ModuleIdx>,
) {
  let mut stack = vec![root];
  while let Some(module_idx) = stack.pop() {
    let Module::Normal(importee) = &modules[module_idx] else { continue };
    let importee_linking_info = &metas[importee.idx];

    if !visited.insert(importee.idx) {
      continue;
    }

    // Only collect modules whose wrapper is declared (i.e. the module is included in the output)
    // and assigned to a chunk. Cross-chunk wrapper imports are registered after this pass.
    if importee_linking_info.is_included
      && esm_wrapper_stmt_is_included_in_live_chunk(
        importee_linking_info,
        importee.idx,
        chunk_graph,
      )
    {
      targets.push(importee.idx);
      continue;
    }

    if importee_linking_info.is_included
      || !matches!(importee.exports_kind, ExportsKind::Esm | ExportsKind::None)
    {
      continue;
    }

    // Importee is a non-included barrel module — traverse its static imports to find included
    // wrapped importees transitively. Preserve recursive DFS order with an explicit LIFO stack:
    // pushing children in reverse keeps source-order visitation left-to-right.
    for rec in importee.import_records.iter().rev() {
      if rec.kind == ImportKind::Import
        && let Some(sub_importee_idx) = rec.resolved_module
      {
        stack.push(sub_importee_idx);
      }
    }
  }
}

fn esm_wrapper_stmt_is_included_in_live_chunk(
  meta: &LinkingMetadata,
  module_idx: ModuleIdx,
  chunk_graph: &ChunkGraph,
) -> bool {
  meta.wrapper_stmt_info.is_some_and(|stmt_info_idx| meta.stmt_info_included.has_bit(stmt_info_idx))
    && module_has_live_chunk(chunk_graph, module_idx)
}

fn module_has_live_chunk(chunk_graph: &ChunkGraph, module_idx: ModuleIdx) -> bool {
  chunk_graph.module_to_chunk[module_idx].is_some_and(|chunk_idx| {
    chunk_graph.post_chunk_optimization_operations.get(&chunk_idx)
      != Some(&PostChunkOptimizationOperation::Removed)
      && chunk_graph.chunk_table[chunk_idx].modules.contains(&module_idx)
  })
}
