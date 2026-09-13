#![forbid(unsafe_code)]

use std::ops::Deref;

use oxc::ast::ast::{Declaration, Statement};
use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ConstExportMeta, ImportKind, ImportRecordIdx,
  ImportRecordMeta, IndexModules, InlineConstMode, Module, ModuleIdx,
  ModuleNamespaceIncludedReason, NormalModule, StmtInfoIdx, StmtInfos, SymbolRef, SymbolRefDb,
  WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{index_vec_ext::IndexVecRefExt, rayon::ParallelIterator as _};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  esm_init_obligations::{
    WrappedEsmInitTarget, WrappedEsmInitTargetContext, collect_order_wrap_esm_init_targets,
    collect_wrapped_esm_init_targets_for_import_record, import_record_has_live_binding_consumer,
    importer_reexports_binding, reexport_record_owns_hop,
  },
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::{
  GenerateStage,
  compute_cross_chunk_links::UsedSymbolRefsView,
  order_wrap_state::{EsmInitOrigin, OrderImportKey, OrderWrapState},
};

/// An artifact whose owner can only read the sealed value.
///
/// Construction is private to this leaf module, and the wrapper exposes neither `DerefMut` nor an
/// unwrap operation. Once a value crosses this boundary, re-owning it cannot make it mutable again.
#[derive(Debug)]
pub struct Sealed<T>(T);

impl<T> Sealed<T> {
  fn new(value: T) -> Self {
    Self(value)
  }
}

impl<T> Deref for Sealed<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/// Final post-chunking facts needed to emit wrapped ESM initialization.
///
/// This artifact is deliberately separate from both link-owned [`LinkingMetadata`] and
/// order-lowering-owned [`OrderWrapState`]. It is computed once after chunk topology and wrapper
/// selection are final, sealed, then shared read-only by cross-chunk linking and module
/// finalization.
#[derive(Debug)]
pub struct FinalEsmInitMetadata {
  modules: FxHashMap<ModuleIdx, ModuleEsmInitMetadata>,
}

impl FinalEsmInitMetadata {
  pub(crate) fn init_is_noop(&self, module_idx: ModuleIdx) -> bool {
    self.modules.get(&module_idx).is_some_and(|metadata| metadata.init_is_noop)
  }

  pub(crate) fn transitive_init_targets(
    &self,
    module_idx: ModuleIdx,
  ) -> Option<&FxHashMap<StmtInfoIdx, Vec<WrappedEsmInitTarget>>> {
    self.modules.get(&module_idx).map(|metadata| &metadata.transitive_init_targets)
  }
}

#[derive(Debug)]
struct ModuleEsmInitMetadata {
  init_is_noop: bool,
  transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<WrappedEsmInitTarget>>,
}

impl GenerateStage<'_> {
  /// Compute the immutable no-op and excluded-statement init facts after final chunk assignment.
  /// This must finish before cross-chunk linking and parallel module finalization.
  pub(super) fn compute_wrapped_esm_init_metadata(
    &self,
    ast_table: &IndexEcmaAst,
    chunk_graph: &ChunkGraph,
    order_state: &OrderWrapState,
    used_symbol_refs: &dyn UsedSymbolRefsView,
  ) -> Sealed<FinalEsmInitMetadata> {
    let keep_names = self.options.keep_names;
    // Off-strict, lowering never mutates the chunk graph, so the liveness guard cannot fire.
    let strict = self.options.is_strict_execution_order_enabled();
    let metas = &self.link_output.metas;
    let modules = &self.link_output.module_table.modules;
    let stmt_infos_vec = &self.link_output.stmt_infos;
    let module_to_chunk = &chunk_graph.module_to_chunk;
    let results = metas
      .par_iter_enumerated()
      .filter_map(|(module_idx, meta)| {
        if !meta.is_included {
          return None;
        }
        let init_target = order_state.esm_init_target(module_idx, meta)?;
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
                order_state,
                stmt_infos_vec,
                symbol_db: &self.link_output.symbol_db,
                constant_value_map: &self.link_output.global_constant_symbol_map,
                inline_const_mode: self.options.optimization.inline_const.map(|config| config.mode),
                used_symbol_refs,
              },
            )
          })
          .unwrap_or_default();
        (is_noop || !targets_by_stmt.is_empty()).then_some((
          module_idx,
          ModuleEsmInitMetadata { init_is_noop: is_noop, transitive_init_targets: targets_by_stmt },
        ))
      })
      .collect::<Vec<_>>();

    Sealed::new(FinalEsmInitMetadata { modules: results.into_iter().collect() })
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
  stmt_infos_vec: &'a IndexStmtInfos,
  symbol_db: &'a SymbolRefDb,
  constant_value_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
  inline_const_mode: Option<InlineConstMode>,
  used_symbol_refs: &'a dyn UsedSymbolRefsView,
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
    Statement::ExportDeclaration(export) => match &export.declaration {
      Declaration::FunctionDeclaration(_) => !keep_names,
      _ => false,
    },
    Statement::ExportNamedDeclaration(_) => true,
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
) -> FxHashMap<StmtInfoIdx, Vec<WrappedEsmInitTarget>> {
  // Shared across all excluded re-export statements of this importer, so a barrel subtree is
  // traversed at most once and each target is attributed to the first statement that reaches
  // it (matching the finalizer's per-module emission dedup).
  let mut visited = FxHashSet::default();
  let mut targets_by_stmt = FxHashMap::<StmtInfoIdx, Vec<WrappedEsmInitTarget>>::default();
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
      // A simulated dynamic-entry facade materializes a namespace object, but it is not an opaque
      // observation of every export. The namespace statement is narrowed to the exports retained
      // by link-time consumers, so re-export init routing must keep using their recorded paths.
      // Only a real link-stage namespace use or semantic order-lowering glue may expand all
      // non-ambiguous exports here.
      let namespace_is_semantically_observed = meta.module_namespace_included_reason.intersects(
        ModuleNamespaceIncludedReason::Unknown
          | ModuleNamespaceIncludedReason::ReExportDynamicExports,
      ) || ctx
        .order_state
        .requires_semantic_namespace(module.namespace_object_ref, |importer_idx| {
          ctx.chunk_graph.module_is_in_live_chunk(importer_idx)
        });
      let namespace_reexport_is_retained = rec.meta.contains(ImportRecordMeta::IsExportStar)
        && meta.namespace_included
        && namespace_is_semantically_observed
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
          // A non-forwarding excluded plain import can still carry importer-local binding demand:
          // a statically folded member read, or a binding this module re-exports that downstream
          // consumers reach through its export surface. Those consumers delegate wholesale to this
          // module's `init_*`, so the wrapper must initialize the leaves that demand selects
          // (issue #10690). Included plain imports emit at their own statement position instead.
          if !stmt_is_included && !is_reexport {
            let record_targets = excluded_plain_import_init_targets(module, meta, ctx, rec_idx);
            if !record_targets.is_empty() {
              targets_by_stmt.entry(stmt_idx).or_default().extend(record_targets);
            }
          }
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
      if namespace_reexport_is_retained && ctx.order_state.is_consumer_local_reexport_route(root) {
        let namespace_targets = ctx
          .order_state
          .consumer_local_namespace_targets(root)
          .expect("consumer-local route should have complete namespace targets");
        targets.extend(namespace_targets.iter().copied().filter(|target| match target {
          WrappedEsmInitTarget::Module(module_idx) => {
            ctx.order_state.esm_init_included_in_live_chunk(
              &ctx.metas[*module_idx],
              *module_idx,
              ctx.chunk_graph,
            )
          }
          WrappedEsmInitTarget::CjsCarrier(key) => {
            ctx.order_state.order_cjs_carrier_included_in_live_chunk(*key, ctx.chunk_graph)
          }
        }));
      } else if ctx.order_wrap {
        // A recorded retained path restricts the hop walk to the chains resolved reads consumed.
        // That is only sound when the path is the record's whole evidence: an included namespace
        // (or forwarded dynamic exports) retains EVERY non-ambiguous export of this star record —
        // including chains no resolved read recorded — so the walk must stay unrestricted there or
        // the off-path pure definers lose their only init call site.
        let retained_reexport_path = if namespace_reexport_is_retained {
          None
        } else {
          overlay
            .filter(|overlay| !overlay.retained_reexport_path.is_empty())
            .map(|overlay| overlay.retained_reexport_path.as_slice())
        };
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

/// Resolve the binding demand of one excluded, non-forwarding plain-import record through the
/// shared per-record router — the same resolver Emit runs for included statements — so the
/// wrapper's excluded-statement metadata owns exactly the leaves and carriers this importer's
/// consumers reach through it. Registration consumes the same metadata, which is what makes every
/// collected wrapper reachable; the live-chunk filter below keeps emission from calling a wrapper
/// that no live chunk declares.
///
/// The router runs only when the record carries binding demand: a live binding consumer
/// (used binding, consumed facade, or member read in this module's included statements) or a
/// binding this module re-exports, which downstream consumers reach through its export surface
/// even when every read was statically folded away. A demand-less record — a side-effect-only
/// import of a side-effect-free module, or dead named imports — was stripped by tree shaking
/// deliberately, and routing it would resurrect that edge (the router initializes a wrapped
/// importee wholesale, matching an included import's evaluation semantics).
fn excluded_plain_import_init_targets(
  module: &NormalModule,
  meta: &LinkingMetadata,
  ctx: &EsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
) -> Vec<WrappedEsmInitTarget> {
  let router_ctx = WrappedEsmInitTargetContext {
    importer: module,
    importer_meta: meta,
    modules: ctx.modules,
    metas: ctx.metas,
    stmt_infos: ctx.stmt_infos_vec,
    symbol_db: ctx.symbol_db,
    constant_value_map: ctx.constant_value_map,
    inline_const_mode: ctx.inline_const_mode,
    order_wrap_state: ctx.order_state,
    // Only order-wrapped modules reach this router, and order wrapping exists only under strict
    // execution order.
    strict_execution_order: true,
  };
  let record_reexports_binding = module.named_imports.iter().any(|(imported_as_ref, import)| {
    import.record_idx == rec_idx && importer_reexports_binding(module, *imported_as_ref)
  });
  if !record_reexports_binding
    && !import_record_has_live_binding_consumer(&router_ctx, rec_idx, |symbol_ref| {
      ctx.used_symbol_refs.contains(&symbol_ref)
    })
  {
    return Vec::new();
  }
  collect_wrapped_esm_init_targets_for_import_record(
    &router_ctx,
    rec_idx,
    |symbol_ref| ctx.used_symbol_refs.contains(&symbol_ref),
    |_| true,
    |forwarding_module_idx| ctx.module_to_chunk[forwarding_module_idx] == Some(ctx.chunk_idx),
  )
  .into_iter()
  .filter(|target| match target {
    WrappedEsmInitTarget::Module(module_idx) => ctx.order_state.esm_init_included_in_live_chunk(
      &ctx.metas[*module_idx],
      *module_idx,
      ctx.chunk_graph,
    ),
    WrappedEsmInitTarget::CjsCarrier(key) => {
      ctx.order_state.order_cjs_carrier_included_in_live_chunk(*key, ctx.chunk_graph)
    }
  })
  .collect()
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
  if order_state.is_consumer_local_reexport_route(importer_idx) {
    return false;
  }
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
  targets: &mut Vec<WrappedEsmInitTarget>,
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
      targets.push(WrappedEsmInitTarget::Module(importee.idx));
    } else {
      for rec in importee.import_records.iter().rev() {
        if let Some(sub_importee_idx) = rec.resolved_module {
          stack.push(sub_importee_idx);
        }
      }
    }
  }
}
