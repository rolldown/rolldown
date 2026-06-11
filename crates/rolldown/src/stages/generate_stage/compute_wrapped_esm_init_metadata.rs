use oxc::ast::ast::{Declaration, Statement};
use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ImportRecordMeta, IndexModules, Module, ModuleIdx,
  NormalModule, StmtInfoIdx, StmtInfos, WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{index_vec_ext::IndexVecRefExt, rayon::ParallelIterator as _};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::GenerateStage;

impl GenerateStage<'_> {
  /// Pre-finalization pass computing, for every included wrapped (`WrapKind::Esm`) module, the
  /// init-call facts the module finalizers consume when emitting `init_*()` calls:
  ///
  /// - [`LinkingMetadata::init_is_noop`]: the module's `__esm` closure body is empty — every
  ///   top-level statement is a hoisted function declaration (lifted out of the closure) or a
  ///   source-less export clause — so call sites can be marked `@__PURE__` and the default
  ///   `dce-only` minifier drops them (and, once the wrapper is unreferenced, the wrapper
  ///   declaration / runtime helper too).
  /// - [`LinkingMetadata::transitive_esm_init_targets`]: per non-included top-level re-export
  ///   statement (`export * from`, `export {x} from`, `export * as ns from`), the wrapped-ESM
  ///   modules whose `init_*()` calls must be emitted in the statement's place, so
  ///   initialization order is preserved when a barrel's re-exports are tree-shaken while
  ///   transitive importees stay included.
  ///
  /// Must run after chunk assignment (`module_to_chunk`) and `on_demand_wrapping` (final wrap
  /// kinds), and before [`Self::finalize_modules`]: modules are finalized in parallel, and an
  /// importer needs its importees' facts while emitting init calls, so neither can be computed
  /// during finalization itself. The transitive targets cannot be computed in the link stage
  /// either: which init calls survive depends on chunk assignment — only same-chunk wrappers
  /// are callable, because cross-chunk wrapper imports are registered only for included
  /// statements.
  pub(super) fn compute_wrapped_esm_init_metadata(
    &mut self,
    ast_table: &IndexEcmaAst,
    chunk_graph: &ChunkGraph,
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
          .map(|(module, chunk_idx)| {
            transitive_esm_init_targets(
              module,
              meta,
              &stmt_infos_vec[module_idx],
              modules,
              metas,
              module_to_chunk,
              chunk_idx,
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

/// For each non-included re-export statement of `module`, the wrapped-ESM modules whose
/// `init_*()` calls the finalizer must emit in the statement's place.
fn transitive_esm_init_targets(
  module: &NormalModule,
  meta: &LinkingMetadata,
  stmt_infos: &StmtInfos,
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
  chunk_idx: ChunkIdx,
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
      // `IsExportStar` / `IsReExportOnly` are set by the scanner exactly on the three
      // re-export statement forms above; plain imports never carry them.
      if !rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly) {
        continue;
      }
      let Some(root) = rec.resolved_module else { continue };
      let mut targets = vec![];
      collect_transitive_esm_init_targets(
        modules,
        metas,
        module_to_chunk,
        chunk_idx,
        root,
        &mut visited,
        &mut targets,
      );
      if !targets.is_empty() {
        targets_by_stmt.entry(stmt_idx).or_default().extend(targets);
      }
    }
  }
  targets_by_stmt
}

/// Find the wrapped-ESM modules an excluded re-export statement must still initialize, by
/// traversing through non-included barrel modules to reach included importees whose wrappers
/// are available in `chunk_idx`.
fn collect_transitive_esm_init_targets(
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

    // Only collect modules in the same chunk whose wrapper is declared (i.e. the module is
    // included in the output).
    if importee_linking_info.is_included && module_to_chunk[importee.idx] == Some(chunk_idx) {
      targets.push(importee.idx);
    } else {
      // Importee is not included (barrel module) — traverse its import records to find
      // included importees transitively.
      // Preserve recursive DFS order with an explicit LIFO stack: pushing children in
      // reverse keeps source-order visitation left-to-right.
      for rec in importee.import_records.iter().rev() {
        if let Some(sub_importee_idx) = rec.resolved_module {
          stack.push(sub_importee_idx);
        }
      }
    }
  }
}
