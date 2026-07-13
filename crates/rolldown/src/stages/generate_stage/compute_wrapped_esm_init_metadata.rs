use oxc::ast::ast::{Declaration, Statement};
use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  IndexModules, Module, ModuleIdx, NormalModule, StmtInfoIdx, StmtInfos, WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{index_vec_ext::IndexVecRefExt, rayon::ParallelIterator as _};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  esm_init_obligations::{collect_order_wrap_esm_init_targets, reexport_record_owns_hop},
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::{
  GenerateStage,
  order_wrap_state::{EsmInitOrigin, OrderImportKey, OrderWrapState},
};

impl GenerateStage<'_> {
  /// Compute no-op wrappers and excluded-statement init targets after final chunk assignment.
  /// This must finish before parallel module finalization.
  pub(super) fn compute_wrapped_esm_init_metadata(
    &mut self,
    ast_table: &IndexEcmaAst,
    chunk_graph: &ChunkGraph,
    order_state: &mut OrderWrapState,
  ) {
    // Classify in parallel (read-only); the cheap write-back stays sequential.
    let keep_names = self.options.keep_names;
    // Off-strict, lowering never mutates the chunk graph, so the liveness guard cannot fire.
    let strict = self.options.is_strict_execution_order_enabled();
    let metas = &self.link_output.metas;
    let modules = &self.link_output.module_table.modules;
    let stmt_infos_vec = &self.link_output.stmt_infos;
    let module_to_chunk = &chunk_graph.module_to_chunk;
    let order_state_view = &*order_state;
    let results = metas
      .par_iter_enumerated()
      .filter_map(|(module_idx, meta)| {
        if !meta.is_included {
          return None;
        }
        let init_target = order_state_view.esm_init_target(module_idx, meta)?;
        let is_noop = init_is_noop(meta, ast_table[module_idx].as_ref(), keep_names);
        let targets_by_stmt = modules[module_idx]
          .as_normal()
          .zip(module_to_chunk[module_idx])
          .filter(|_| !strict || chunk_graph.module_is_in_live_chunk(module_idx))
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
                order_wrap: matches!(init_target.origin, EsmInitOrigin::ExecutionOrder),
                execution_dependencies: &meta.execution_dependencies,
                order_state: order_state_view,
              },
            )
          })
          .unwrap_or_default();
        (is_noop || !targets_by_stmt.is_empty()).then_some((
          module_idx,
          init_target.origin,
          is_noop,
          targets_by_stmt,
        ))
      })
      .collect::<Vec<_>>();

    for (module_idx, origin, is_noop, targets_by_stmt) in results {
      match origin {
        EsmInitOrigin::Interop => {
          let meta = &mut self.link_output.metas[module_idx];
          meta.init_is_noop = is_noop;
          meta.transitive_esm_init_targets = targets_by_stmt;
        }
        EsmInitOrigin::ExecutionOrder => {
          order_state.set_order_init_metadata(module_idx, is_noop, targets_by_stmt);
        }
      }
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
  order_state: &'a OrderWrapState,
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
    let stmt_is_included = meta.stmt_info_included.has_bit(stmt_idx);
    if stmt_is_included && !ctx.order_wrap {
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
      let overlay = ctx.order_state.import_overlay(OrderImportKey {
        importer: module.idx,
        statement: stmt_idx,
        record: rec_idx,
      });
      let namespace_reexport_is_retained = rec.meta.contains(ImportRecordMeta::IsExportStar)
        && meta.namespace_included
        && (ctx.metas[root].has_dynamic_exports
          || meta.star_export_record_by_name.iter().any(|(name, owner)| {
            *owner == rec_idx && meta.sorted_and_non_ambiguous_resolved_exports.contains_key(name)
          }));
      if ctx.order_wrap {
        if !order_wrap_record_forwards(
          ctx.order_state,
          ctx.execution_dependencies,
          module.idx,
          rec_idx,
          root,
          is_reexport,
          ReexportRetentionEvidence {
            statement: stmt_is_included,
            overlay: overlay.is_some(),
            namespace: namespace_reexport_is_retained,
          },
        ) {
          continue;
        }
        if stmt_is_included
          && overlay.is_none_or(|overlay| overlay.retained_reexport_path.is_empty())
        {
          continue;
        }
      } else if !is_reexport {
        continue;
      }
      let mut targets = vec![];
      if ctx.order_wrap {
        let retained_reexport_path = overlay
          .filter(|overlay| !overlay.retained_reexport_path.is_empty())
          .map(|overlay| overlay.retained_reexport_path.as_slice());
        let mut retained_path_visited = FxHashSet::default();
        collect_order_wrap_esm_init_targets(
          ctx.modules,
          ctx.metas,
          ctx.chunk_graph,
          ctx.order_state,
          ctx.chunk_idx,
          root,
          retained_reexport_path,
          if retained_reexport_path.is_some() { &mut retained_path_visited } else { &mut visited },
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

/// Whether an order-wrapped importer's `init_*` must forward through this static-import record.
///
/// It forwards on either of two conditions:
/// - **execution dependency** — the record's target is a live execution dependency of the importer
///   (a side-effecting module the importer evaluates); or
/// - **owns a retained re-export hop** — [`reexport_record_owns_hop`], the shared ownership
///   predicate, plus proof that tree-shaking retained the statement, lowering recorded an import
///   overlay for a consumed path, or the record contributes a non-ambiguous export to the included
///   namespace (or forwards dynamic exports whose names are not statically enumerable). Merely
///   wrapping a barrel must not resurrect an excluded pure re-export: wrap-all and on-demand may
///   select different wrapper sets, but they must preserve the same tree-shaking result.
///
/// The namespace exception is deliberately per-record: `export *` does not forward `default`, a
/// local export can shadow a star export, and conflicting star exports are absent from the
/// non-ambiguous namespace. Treating every star from a namespace-included module as retained would
/// resurrect those dead re-exports.
#[derive(Clone, Copy)]
struct ReexportRetentionEvidence {
  statement: bool,
  overlay: bool,
  namespace: bool,
}

impl ReexportRetentionEvidence {
  fn any(self) -> bool {
    self.statement || self.overlay || self.namespace
  }
}

fn order_wrap_record_forwards(
  order_state: &OrderWrapState,
  execution_dependencies: &rolldown_utils::indexmap::FxIndexSet<ModuleIdx>,
  importer_idx: ModuleIdx,
  rec_idx: ImportRecordIdx,
  root: ModuleIdx,
  is_reexport: bool,
  retention: ReexportRetentionEvidence,
) -> bool {
  execution_dependencies.contains(&root)
    || (retention.any()
      && reexport_record_owns_hop(order_state, importer_idx, rec_idx, is_reexport))
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
