use crate::{
  chunk_graph::ChunkGraph,
  esm_init_obligations::{
    WrappedEsmInitTarget, WrappedEsmInitTargetContext,
    collect_wrapped_esm_init_targets_for_module_namespace,
  },
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  utils::external_import_interop::import_record_needs_interop,
};
use itertools::Itertools;
use oxc::ast::ast::{Declaration, ExportDefaultDeclarationKind, Statement};
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, EntryPointKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, IndexModules, ModuleIdx, OutputFormat, PostChunkOptimizationOperation,
  RuntimeHelper, StmtInfoIdx, SymbolRef, SymbolRefDb, UsedSymbolRefsBuilder, WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::StatementExt;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  chunk_optimizer::RuntimeMergeCascade,
  order_analysis::{OrderAnalysis, OrderWrapPlan},
  order_wrap_state::{
    OrderCjsCarrierKey, OrderCjsCarrierSpec, OrderImportKey, OrderImportOverlay, OrderWrapState,
    SimulatedFacadeNamespaceExport,
  },
};

/// The one export name that makes a module namespace a thenable to the promise resolution
/// procedure, and so unsafe to hand to a `import('./host.js').then(...)` rewrite.
const THEN: &str = "then";

#[derive(Clone, Copy)]
struct LiveDynamicImporter {
  module_idx: ModuleIdx,
  /// The chunk hosting the importer, i.e. where the rewritten call site would carry the trigger.
  /// `None` means the importer has no chunk of its own, so it cannot carry one.
  trigger_chunk: Option<ChunkIdx>,
}

pub(super) struct OrderLoweringInput<'a> {
  pub(super) plan: &'a OrderWrapPlan,
  pub(super) modules: &'a IndexModules,
  pub(super) linking: &'a LinkingMetadataVec,
  pub(super) statements: &'a IndexStmtInfos,
  pub(super) asts: &'a IndexEcmaAst,
  pub(super) keep_names: bool,
  pub(super) export_chains: &'a FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub(super) star_reexport_records_by_imported_symbol:
    &'a FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
  pub(super) used_symbols: &'a UsedSymbolRefsBuilder,
  pub(super) cyclic_modules: &'a FxHashSet<ModuleIdx>,
  pub(super) tree_shaking: bool,
}

pub(super) struct ConsumerLocalReexportPlan {
  modules: Vec<ModuleIdx>,
  carriers: Vec<OrderCjsCarrierPlan>,
}

struct OrderCjsCarrierPlan {
  key: OrderCjsCarrierKey,
  importee: ModuleIdx,
  namespace_ref: SymbolRef,
  importee_wrapper_ref: SymbolRef,
  mapped_symbols: Vec<SymbolRef>,
  needs_to_esm: bool,
  is_node_mode: bool,
  eager: bool,
}

/// Whether an execution-order wrapper is only a routing waypoint for re-export initialization.
///
/// Unlike `init_is_noop`, this deliberately ignores import/re-export lowering glue: that glue is
/// consumer-dependent and retained leaf initialization must be routed from the consuming record,
/// not installed into a shared pure barrel wrapper. Local executable statements, generated missing
/// export assignments, `keepNames` calls, and unconditional execution dependencies make the
/// wrapper non-transparent.
pub(super) fn order_wrapper_is_reexport_transparent(
  meta: &LinkingMetadata,
  ast: Option<&EcmaAst>,
  keep_names: bool,
) -> bool {
  matches!(
    meta.concatenated_wrapped_module_kind,
    rolldown_common::ConcatenateWrappedModuleKind::None
  ) && meta.shimmed_missing_exports.is_empty()
    && meta.execution_dependencies.is_empty()
    && ast.is_some_and(|ast| {
      ast.program().body.iter().all(|stmt| statement_has_no_local_wrapper_body(stmt, keep_names))
    })
}

fn statement_has_no_local_wrapper_body(stmt: &Statement, keep_names: bool) -> bool {
  // Static import/re-export statements may lower to init forwarding or namespace glue, but that
  // work is routed per consumer for transparent wrappers and is not a module-local executable body.
  if stmt.is_module_declaration_with_source() {
    return true;
  }
  match stmt {
    Statement::FunctionDeclaration(_) => !keep_names,
    Statement::ExportDefaultDeclaration(export) => {
      matches!(export.declaration, ExportDefaultDeclarationKind::FunctionDeclaration(_))
        && !keep_names
    }
    Statement::ExportNamedDeclaration(export) => match &export.declaration {
      None => true,
      Some(Declaration::FunctionDeclaration(_)) => !keep_names,
      Some(_) => false,
    },
    _ => false,
  }
}

fn statement_is_direct_reexport(stmt: &Statement) -> bool {
  match stmt {
    Statement::ExportAllDeclaration(_) => true,
    Statement::ExportNamedDeclaration(export) => export.source.is_some(),
    _ => false,
  }
}

pub(super) fn synchronous_cycle_modules(modules: &IndexModules) -> FxHashSet<ModuleIdx> {
  let mut graph = petgraph::prelude::DiGraphMap::<ModuleIdx, ()>::new();
  for module in modules.iter().filter_map(|module| module.as_normal()) {
    graph.add_node(module.idx);
    for rec in &module.import_records {
      if matches!(rec.kind, ImportKind::Import | ImportKind::Require)
        && let Some(importee_idx) = rec.resolved_module
        && modules[importee_idx].is_normal()
      {
        graph.add_edge(module.idx, importee_idx, ());
      }
    }
  }

  let mut cyclic = FxHashSet::default();
  for component in petgraph::algo::tarjan_scc(&graph) {
    if component.len() > 1 || graph.contains_edge(component[0], component[0]) {
      cyclic.extend(component);
    }
  }
  cyclic
}

pub(super) fn consumer_local_reexport_plan(
  input: &OrderLoweringInput<'_>,
  state: &OrderWrapState,
) -> ConsumerLocalReexportPlan {
  let mut modules = Vec::new();
  let mut accepted_modules = FxHashSet::default();
  let mut carriers = Vec::new();
  // `require()` consumes an ESM module as an opaque namespace. The importer-local resolver does
  // not yet lower require call sites into a complete namespace-target sequence, so keep those
  // barrels on the existing monolithic wrapper path. This is intentionally module-wide and
  // conservative: a dead or deferred require may reject an otherwise safe candidate, but it can
  // never expose an uninitialized carrier namespace at runtime.
  let required_modules = input
    .modules
    .iter()
    .filter_map(|module| module.as_normal())
    .flat_map(|module| &module.import_records)
    .filter(|rec| rec.kind == ImportKind::Require)
    .filter_map(|rec| rec.resolved_module)
    .collect::<FxHashSet<_>>();
  let ordered_modules = input
    .modules
    .iter_enumerated()
    .filter_map(|(module_idx, module)| module.as_normal().is_some().then_some(module_idx))
    .sorted_unstable_by_key(|idx| input.modules[*idx].exec_order())
    .collect_vec();

  // Acceptance is a small monotone fixpoint. Module execution order is not guaranteed to put an
  // inner barrel before every outer forwarder, while an outer module may only ignore an importee's
  // transitive side-effect bit after that importee has itself become a consumer-local waypoint.
  // Each round accepts at least one new module or stops, so this is bounded by the plan size.
  loop {
    let accepted_before = modules.len();
    for &module_idx in &ordered_modules {
      if accepted_modules.contains(&module_idx) {
        continue;
      }
      if state.is_consumer_local_reexport_route(module_idx)
        || !input.tree_shaking
        || input.cyclic_modules.contains(&module_idx)
        || required_modules.contains(&module_idx)
      {
        continue;
      }
      let meta = &input.linking[module_idx];
      if meta.has_dynamic_exports
        || meta.is_tla_or_contains_tla_dependency
        || !matches!(
          meta.concatenated_wrapped_module_kind,
          rolldown_common::ConcatenateWrappedModuleKind::None
        )
        || !meta.shimmed_missing_exports.is_empty()
      {
        continue;
      }
      let Some(ast) = input.asts[module_idx].as_ref() else {
        continue;
      };
      if ast.program().body.is_empty()
        || !ast.program().body.iter().all(statement_is_direct_reexport)
      {
        continue;
      }
      let Some(module) = input.modules[module_idx].as_normal() else {
        continue;
      };

      let mut module_carriers = Vec::new();
      let mut forwards_consumer_local_route = false;
      let mut supported = true;
      for (stmt_idx, stmt_info) in input.statements[module_idx].iter_enumerated() {
        let stmt_is_included = meta.stmt_info_included.has_bit(stmt_idx);
        for &rec_idx in &stmt_info.import_records {
          let rec = &module.import_records[rec_idx];
          if rec.kind != ImportKind::Import {
            supported = false;
            break;
          }
          let Some(importee_idx) = rec.resolved_module else {
            supported = false;
            break;
          };
          let Some(importee) = input.modules[importee_idx].as_normal() else {
            supported = false;
            break;
          };
          match input.linking[importee_idx].wrap_kind() {
            WrapKind::Cjs => {
              // An excluded pure CJS re-export has no output-side binding demand and needs no
              // carrier. If it later becomes retained, tree shaking marks this statement included
              // before generate-stage routing is built.
              if !stmt_is_included {
                continue;
              }
              if rec.meta.contains(ImportRecordMeta::IsExportStar) {
                supported = false;
                break;
              }
              let Some(importee_wrapper_ref) = input.linking[importee_idx].wrapper_ref else {
                supported = false;
                break;
              };
              let mut mapped_symbols = vec![rec.namespace_ref];
              for (imported_as_ref, named_import) in
                module.named_imports.iter().filter(|(_, import)| import.record_idx == rec_idx)
              {
                if !mapped_symbols.contains(imported_as_ref) {
                  mapped_symbols.push(*imported_as_ref);
                }
                if !mapped_symbols.contains(&named_import.imported_as) {
                  mapped_symbols.push(named_import.imported_as);
                }
              }
              for (name, local_export) in &module.named_exports {
                if module
                  .named_imports
                  .get(&local_export.referenced)
                  .is_some_and(|import| import.record_idx == rec_idx)
                {
                  if !mapped_symbols.contains(&local_export.referenced) {
                    mapped_symbols.push(local_export.referenced);
                  }
                  if let Some(resolved_export) = meta.resolved_exports.get(name)
                    && !mapped_symbols.contains(&resolved_export.symbol_ref)
                  {
                    mapped_symbols.push(resolved_export.symbol_ref);
                  }
                }
              }
              module_carriers.push(OrderCjsCarrierPlan {
                key: OrderCjsCarrierKey { importer: module_idx, record: rec_idx },
                importee: importee_idx,
                namespace_ref: rec.namespace_ref,
                importee_wrapper_ref,
                mapped_symbols,
                needs_to_esm: import_record_needs_interop(module, rec_idx),
                is_node_mode: module.should_consider_node_esm_spec_for_static_import(),
                // A side-effectful CJS importee is eager for every barrel consumer only when the
                // barrel itself retains this re-export record unconditionally. In particular, a
                // user-declared `moduleSideEffects: false` barrel may retain the record globally
                // for a lazy binding consumer without making it an execution dependency of an
                // unrelated eager binding route.
                eager: module.side_effects.has_side_effects()
                  && stmt_info.eval_flags.has_side_effect_for_tree_shaking(),
              });
            }
            WrapKind::None | WrapKind::Esm => {
              let importee_is_consumer_local = accepted_modules.contains(&importee_idx)
                || state.is_consumer_local_reexport_route(importee_idx);
              forwards_consumer_local_route |= importee_is_consumer_local;
              // A previously accepted inner routing waypoint may report transitive side effects
              // because it contains eager CJS carriers. Those effects are already represented as
              // per-record eager obligations, so an outer re-export-only barrel can forward them
              // without regaining a monolithic body. A genuinely effectful leaf still rejects the
              // optimization.
              if importee.side_effects.has_side_effects() && !importee_is_consumer_local {
                supported = false;
                break;
              }
            }
          }
        }
        if !supported {
          break;
        }
      }

      // Carrier-free direct barrels are part of the same route when they forward an inner
      // carrierized barrel. Marking every supported re-export-only waypoint keeps an arbitrarily
      // deep chain importer-local; its non-CJS leaves are side-effect-free by the check above, so
      // there is no module-local evaluation work to preserve here.
      if supported && (!module_carriers.is_empty() || forwards_consumer_local_route) {
        accepted_modules.insert(module_idx);
        modules.push(module_idx);
        carriers.extend(module_carriers);
      }
    }
    if modules.len() == accepted_before {
      break;
    }
  }

  ConsumerLocalReexportPlan { modules, carriers }
}

fn apply_consumer_local_reexport_plan(
  input: &OrderLoweringInput<'_>,
  output: &mut OrderLoweringOutput<'_>,
  runtime_helper: RuntimeHelper,
  plan: &ConsumerLocalReexportPlan,
) {
  for &module_idx in &plan.modules {
    output.state.set_consumer_local_reexport_route(module_idx);
  }
  for (carrier_index, carrier) in plan.carriers.iter().enumerate() {
    let module = input.modules[carrier.key.importer]
      .as_normal()
      .expect("order CJS carrier owner should be a normal module");
    let wrapper_ref = output.symbols.create_facade_root_symbol_ref(
      carrier.key.importer,
      &format!("init_{}_cjs_{carrier_index}", module.repr_name),
    );
    let mut runtime_helpers = runtime_helper;
    if carrier.needs_to_esm {
      runtime_helpers.insert(RuntimeHelper::ToEsm);
    }
    output.state.insert_order_cjs_carrier(
      carrier.key,
      OrderCjsCarrierSpec {
        importee: carrier.importee,
        wrapper_ref,
        namespace_ref: carrier.namespace_ref,
        eager: carrier.eager,
        needs_to_esm: carrier.needs_to_esm,
        is_node_mode: carrier.is_node_mode,
      },
      vec![carrier.importee_wrapper_ref],
      runtime_helpers,
    );
    for &symbol_ref in &carrier.mapped_symbols {
      output.state.map_order_cjs_carrier_symbol(symbol_ref, carrier.key);
    }
  }
}

pub(super) fn apply_consumer_local_reexport_plan_probe(
  state: &mut OrderWrapState,
  plan: &ConsumerLocalReexportPlan,
) {
  for &module_idx in &plan.modules {
    state.set_consumer_local_reexport_route(module_idx);
  }
  for carrier in &plan.carriers {
    state.insert_order_cjs_carrier_probe(
      carrier.key,
      OrderCjsCarrierSpec {
        importee: carrier.importee,
        wrapper_ref: carrier.namespace_ref,
        namespace_ref: carrier.namespace_ref,
        eager: carrier.eager,
        needs_to_esm: carrier.needs_to_esm,
        is_node_mode: carrier.is_node_mode,
      },
    );
    for &symbol_ref in &carrier.mapped_symbols {
      state.map_order_cjs_carrier_symbol(symbol_ref, carrier.key);
    }
  }
}

struct OrderLoweringOutput<'a> {
  symbols: &'a mut SymbolRefDb,
  state: &'a mut OrderWrapState,
}

pub(super) struct FrozenReexportUsage {
  root_paths: FxHashMap<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>,
  nested_records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  consumed_facades: FxHashSet<SymbolRef>,
}

impl FrozenReexportUsage {
  pub(super) fn nested_records(&self) -> &FxHashSet<(ModuleIdx, ImportRecordIdx)> {
    &self.nested_records
  }

  pub(super) fn consumed_facades(&self) -> &FxHashSet<SymbolRef> {
    &self.consumed_facades
  }
}

impl GenerateStage<'_> {
  /// Returns whether the chunk topology changed.
  pub(super) fn apply_order_wraps(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    analysis: &OrderAnalysis,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    order_state: &mut OrderWrapState,
  ) -> bool {
    let plan = &analysis.plan;
    if plan.is_empty() {
      // Entry-trigger facades are needed even with an empty plan: a pure interop graph can
      // still share one entry's chunk with another entry. With nothing order-wrapped, the only
      // candidates are interop-wrapped entries, so a pure-ESM graph has none and asks nothing.
      //
      // The three passes hoisted ahead of the query below are skipped here, which is safe only
      // because an empty plan makes each one a no-op: nothing demands an order-wrap runtime
      // helper, restoring is filtered by the plan, and `finalize_chunk_plan` already ran the
      // namespace pass against this same default `order_state`.
      let candidates = self.entry_facade_candidates(plan);
      if candidates.is_empty() {
        return false;
      }
      let import_edges = self.entry_facade_import_edges(chunk_graph, used_symbol_refs, order_state);
      if !self.create_order_wrap_entry_facades(chunk_graph, candidates, &import_edges, order_state)
      {
        return false;
      }
      order_state.compute_runtime_symbol_closure(
        &self.link_output.runtime,
        &self.link_output.stmt_infos[self.link_output.runtime.id()],
        &self.link_output.symbol_db,
      );
      chunk_graph.sort_chunk_modules(self.link_output, self.options);
      self.renumber_live_chunks(chunk_graph);
      return true;
    }

    let runtime_helper = self.esm_runtime_helper();
    let code_splitting_disabled = self.options.code_splitting.is_disabled();
    let cyclic_modules = synchronous_cycle_modules(&self.link_output.module_table.modules);
    let input = OrderLoweringInput {
      plan,
      modules: &self.link_output.module_table.modules,
      linking: &self.link_output.metas,
      statements: &self.link_output.stmt_infos,
      asts: &self.ast_table,
      keep_names: self.options.keep_names,
      export_chains: &self.link_output.normal_symbol_exports_chain_map,
      star_reexport_records_by_imported_symbol: &self
        .link_output
        .star_reexport_records_by_imported_symbol,
      used_symbols: used_symbol_refs,
      cyclic_modules: &cyclic_modules,
      tree_shaking: self.options.treeshake.is_some(),
    };
    let mut output =
      OrderLoweringOutput { symbols: &mut self.link_output.symbol_db, state: order_state };
    lower_order_state(&input, &mut output, runtime_helper, code_splitting_disabled);
    let consumer_local_namespace_targets = self
      .link_output
      .module_table
      .modules
      .iter_enumerated()
      .filter(|(module_idx, _)| order_state.is_consumer_local_reexport_route(*module_idx))
      .filter_map(|(module_idx, module)| {
        let module = module.as_normal()?;
        let targets = collect_wrapped_esm_init_targets_for_module_namespace(
          &WrappedEsmInitTargetContext {
            importer: module,
            importer_meta: &self.link_output.metas[module_idx],
            modules: &self.link_output.module_table.modules,
            metas: &self.link_output.metas,
            stmt_infos: &self.link_output.stmt_infos,
            symbol_db: &self.link_output.symbol_db,
            constant_value_map: &self.link_output.global_constant_symbol_map,
            inline_const_mode: self.options.optimization.inline_const.map(|config| config.mode),
            order_wrap_state: order_state,
            strict_execution_order: true,
          },
          |_| true,
        );
        Some((module_idx, targets))
      })
      .collect_vec();
    for (module_idx, targets) in consumer_local_namespace_targets {
      order_state.set_consumer_local_namespace_targets(module_idx, targets);
    }
    let runtime_idx = self.link_output.runtime.id();
    order_state.compute_runtime_symbol_closure(
      &self.link_output.runtime,
      &self.link_output.stmt_infos[runtime_idx],
      &self.link_output.symbol_db,
    );
    self.place_order_wrap_modules(chunk_graph, plan, order_state);
    // Restoring runs before everything below it. A restored facade is an optimizer-removed chunk
    // brought back to life, so it changes both the live-chunk count the runtime placement reads
    // and the edge set the facade decision reads — and `compute_chunk_imports` skips removed
    // chunks, so asking first would hide the edge from a revived facade to the chunk hosting its
    // implementation, leaving an entry that shares that chunk with an inline trigger the restored
    // facade's load then fires. Restoring is not itself a candidate for the necessity gate: it
    // exists because a chunk can host only one entry's top-level trigger, so an entry merged into
    // another entry's chunk has no trigger at all without its own file.
    if self.restore_order_wrap_entry_facades(chunk_graph, plan, order_state) {
      // An optimizer-eliminated facade that stays collapsed gains the same exact simulated
      // namespace requirement as a facade collapsed by the creation pass below. Close any late
      // runtime-helper dependencies before runtime placement and final cross-chunk linking.
      order_state.compute_runtime_symbol_closure(
        &self.link_output.runtime,
        &self.link_output.stmt_infos[runtime_idx],
        &self.link_output.symbol_db,
      );
    }
    // Placement runs before the facade decision so that decision can ask the real link computation
    // which chunks import which: the order wrappers demand runtime helpers, and resolving a
    // depended symbol to its chunk requires the runtime module to already sit in one. The merge
    // re-proof this normalizes for is deferred to `fold_runtime_chunk_after_order_lowering` below,
    // because it counts the runtime chunk's consumers and must see the final facade topology.
    let fold_runtime_chunk = self.ensure_runtime_module_for_order_wraps(chunk_graph);
    // Refresh the namespace facts against the lowered order state before querying the edges.
    // `finalize_chunk_plan` re-runs this once more after lowering anyway; doing it here too means
    // the query sees the namespaces the final link pass will see, instead of the provisional
    // pre-lowering set, so a namespace an import overlay demands cannot become an edge the query
    // missed. Facades do not move modules between chunks, so the result is the same before and
    // after they are created.
    self.finalized_module_namespace_ref_usage(chunk_graph, order_state);
    // Collecting the candidates reads only the module graph, so it can run before the edges and
    // spare the query entirely when no entry could need a facade in the first place.
    let candidates = self.entry_facade_candidates(plan);
    if !candidates.is_empty() {
      let import_edges = self.entry_facade_import_edges(chunk_graph, used_symbol_refs, order_state);
      self.create_order_wrap_entry_facades(chunk_graph, candidates, &import_edges, order_state);
      // `create_order_wrap_entry_facades` may replace a newly-created dynamic facade with a
      // call-site trigger. Its simulated namespace adds late runtime-helper demand after the
      // initial closure above (`__exportAll`, plus `__reExport` when the finalizer will merge an
      // external star), so close the dependency set once more before placement and final
      // cross-chunk linking consume it.
      order_state.compute_runtime_symbol_closure(
        &self.link_output.runtime,
        &self.link_output.stmt_infos[runtime_idx],
        &self.link_output.symbol_db,
      );
    }
    if fold_runtime_chunk {
      self.fold_runtime_chunk_after_order_lowering(chunk_graph, order_state);
    }
    chunk_graph.sort_chunk_modules(self.link_output, self.options);
    self.renumber_live_chunks(chunk_graph);
    true
  }

  /// The chunk->chunk static import edges the entry-facade decision is taken against.
  ///
  /// Computed from the fully lowered order state through the same cross-chunk link pass that
  /// produces the final edges, so the decision reads facts rather than a re-derived approximation
  /// of them. See [`GenerateStage::lowered_static_import_edges`].
  fn entry_facade_import_edges(
    &self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefsBuilder,
    order_state: &OrderWrapState,
  ) -> IndexVec<ChunkIdx, FxHashSet<ChunkIdx>> {
    if self.options.code_splitting.is_disabled() {
      // A single-chunk build has no other chunk to load the entry's implementation, and
      // `create_order_wrap_entry_facades` bails out on it anyway.
      return index_vec![FxHashSet::default(); chunk_graph.chunk_table.len()];
    }
    let final_esm_init_metadata =
      self.compute_wrapped_esm_init_metadata(&self.ast_table, chunk_graph, order_state);
    self.lowered_static_import_edges(
      chunk_graph,
      used_symbol_refs,
      order_state,
      &final_esm_init_metadata,
    )
  }

  fn place_order_wrap_modules(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &OrderWrapPlan,
    order_state: &mut OrderWrapState,
  ) {
    // Plan members are included user modules, so chunk assignment already placed every one.
    // Sorted so synthetic-statement registration order (which feeds deconfliction naming)
    // stays deterministic.
    let order_wrapped_modules = plan
      .modules()
      .filter(|module_idx| order_state.has_order_wrapper(*module_idx))
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();
    for module_idx in order_wrapped_modules {
      let chunk_idx =
        chunk_graph.module_to_chunk[module_idx].expect("order-wrapped module should have a chunk");
      order_state.assign_order_wrapper_chunk(module_idx, chunk_idx);
    }
    let carriers = order_state.order_cjs_carrier_keys().collect_vec();
    for key in carriers {
      let importee =
        order_state.order_cjs_carrier(key).expect("order CJS carrier should exist").importee;
      let chunk_idx = chunk_graph.module_to_chunk[importee]
        .expect("order CJS carrier importee should have a chunk");
      order_state.assign_order_cjs_carrier_chunk(key, chunk_idx);
    }
  }

  /// Entries whose inline `init_E()` trigger might have to move into a facade — an order-wrapped
  /// entry, or an interop-wrapped one, whether or not anything imports it.
  ///
  /// Reads only the module graph, never the chunk topology, so the caller can collect these before
  /// computing the import edges and skip that query entirely when there is nothing to decide.
  fn entry_facade_candidates(&self, plan: &OrderWrapPlan) -> Vec<ModuleIdx> {
    if self.options.code_splitting.is_disabled() {
      return vec![];
    }
    let mut candidates = plan
      .modules()
      .filter(|module_idx| self.link_output.entries.contains_key(module_idx))
      .collect_vec();
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      let meta = &self.link_output.metas[module.idx];
      if !meta.is_included {
        continue;
      }
      candidates.extend(module.import_records.iter().filter_map(|rec| {
        if !matches!(rec.kind, ImportKind::Import | ImportKind::Require) {
          return None;
        }
        let importee_idx = rec.resolved_module?;
        (self.link_output.entries.contains_key(&importee_idx)
          && meta.execution_dependencies.contains(&importee_idx)
          && !matches!(self.link_output.metas[importee_idx].wrap_kind(), WrapKind::None))
        .then_some(importee_idx)
      }));
    }
    candidates.extend(self.link_output.entries.keys().copied().filter(|entry_module_idx| {
      !matches!(self.link_output.metas[*entry_module_idx].wrap_kind(), WrapKind::None)
    }));
    candidates
  }

  fn create_order_wrap_entry_facades(
    &self,
    chunk_graph: &mut ChunkGraph,
    facade_candidates: Vec<ModuleIdx>,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
    order_state: &mut OrderWrapState,
  ) -> bool {
    if self.options.code_splitting.is_disabled() {
      return false;
    }

    let mut imported_chunks = FxHashSet::default();
    for (chunk_idx, importee_chunks) in import_edges.iter_enumerated() {
      imported_chunks
        .extend(importee_chunks.iter().copied().filter(|importee| *importee != chunk_idx));
    }
    // A dynamic import evaluates its target's chunk, so an inline entry trigger hosted there
    // would run the entry's whole program during that load (e.g. a manual group placing a
    // dynamic target next to an entry). The static edge set cannot see these loads; collect
    // the cross-chunk dynamic-import targets directly.
    let mut dynamic_target_modules_by_chunk: FxHashMap<ChunkIdx, FxHashSet<ModuleIdx>> =
      FxHashMap::default();
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      if !self.link_output.metas[module.idx].is_included {
        continue;
      }
      let importer_chunk = chunk_graph.module_to_chunk[module.idx];
      for rec in &module.import_records {
        // Only records the emitted code still executes count as loads. Tree shaking flags a
        // dynamic record whose entry it dropped as `DeadDynamicImport`, and the finalizer rewrites
        // exactly those to an inert `Promise.resolve().then(...)` stub that loads nothing — so a
        // dead record must not force the split. The importee can still be included through a
        // static route, which is precisely when it may share an entry's chunk. Same exclusion as
        // `dynamic_already_loaded`.
        if rec.kind != ImportKind::DynamicImport
          || rec.meta.contains(ImportRecordMeta::DeadDynamicImport)
        {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if !self.link_output.module_table[importee_idx].is_normal()
          || !self.link_output.metas[importee_idx].is_included
        {
          continue;
        }
        let Some(importee_chunk) = chunk_graph.module_to_chunk[importee_idx] else { continue };
        if importer_chunk == Some(importee_chunk) {
          continue;
        }
        dynamic_target_modules_by_chunk.entry(importee_chunk).or_default().insert(importee_idx);
      }
    }
    // An entry's `init_E()` trigger may stay inline at the top of the chunk hosting `E` exactly
    // while that chunk is loaded only to enter `E`. Once anything else can load it — another chunk
    // imports it, or it also hosts some other chunk's dynamic-import target — the trigger has to
    // move into a facade or `E`'s program runs during that unrelated load.
    //
    // This is the same question for an order-wrapped entry and for an interop-wrapped one, so all
    // candidate sources are gated by it. It used to be asked only of interop entries, and only in
    // on-demand mode; an order-wrapped entry always split, which cost one extra chunk per entry
    // (including every dynamic entry) even when nothing but the entry itself could load the chunk.
    let mut entries_to_split = facade_candidates
      .into_iter()
      .filter(|entry_module_idx| {
        chunk_graph.entry_module_to_entry_chunk.get(entry_module_idx).is_some_and(
          |entry_chunk_idx| {
            imported_chunks.contains(entry_chunk_idx)
              // A dynamic import of the entry module itself must run its program, so only
              // other hosted targets force the split.
              || dynamic_target_modules_by_chunk.get(entry_chunk_idx).is_some_and(|targets| {
                targets.iter().any(|target| target != entry_module_idx)
              })
          },
        )
      })
      .collect_vec();
    entries_to_split.sort_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order());
    entries_to_split.dedup();

    let dynamic_importers =
      self.live_dynamic_importers(chunk_graph, entries_to_split.iter().copied());

    let mut created = false;
    for entry_module_idx in entries_to_split {
      let Some(entry_chunk_idx) =
        chunk_graph.entry_module_to_entry_chunk.get(&entry_module_idx).copied()
      else {
        continue;
      };
      if matches!(
        chunk_graph.post_chunk_optimization_operations.get(&entry_chunk_idx),
        Some(PostChunkOptimizationOperation::Removed)
      ) {
        continue;
      }

      let importer_sites = &dynamic_importers[&entry_module_idx];
      let importer_chunks =
        importer_sites.iter().map(|importer| importer.trigger_chunk).collect_vec();
      let entry_meta = &self.link_output.metas[entry_module_idx];
      let collapse_has_sync_init_targets =
        if let Some(targets) = order_state.consumer_local_namespace_targets(entry_module_idx) {
          targets.iter().all(|target| match target {
            WrappedEsmInitTarget::Module(module_idx) => order_state
              .esm_init_target(*module_idx, &self.link_output.metas[*module_idx])
              .is_some_and(|target| !target.tla_tainted),
            WrappedEsmInitTarget::CjsCarrier(_) => true,
          })
        } else {
          order_state
            .esm_init_target(entry_module_idx, entry_meta)
            .is_some_and(|target| !target.tla_tainted)
        };
      let collapse_carries_trigger = collapse_has_sync_init_targets
          && !entry_meta.is_tla_or_contains_tla_dependency
          && !importer_sites.is_empty()
          && importer_sites.iter().all(|importer| importer.trigger_chunk.is_some())
          // A facade restored after chunk optimization is also an entry chunk, but its module
          // lives in a different implementation chunk. Only collapse an entry chunk that still
          // owns the dynamic entry implementation; otherwise this would merely turn the empty
          // facade into an empty common chunk and leave the entry mapping pointing at it.
          && chunk_graph.chunk_table[entry_chunk_idx].modules.contains(&entry_module_idx)
          && matches!(
            chunk_graph.chunk_table[entry_chunk_idx].kind,
            ChunkKind::EntryPoint { meta, module, .. }
              if module == entry_module_idx && meta == ChunkMeta::DynamicImported
          )
          // Entry-level external stars render on the facade chunk, with format-specific behavior
          // that a module-local simulated namespace does not reproduce. ESM would lose its
          // chunk-level `export *`; CJS-like formats would replace a deduplicated `Object.keys`
          // merge with per-record `__reExport` calls (which also differ for primitive exports).
          // The chunk fact includes direct and transitive star chains, so keep either shape.
          && chunk_graph.chunk_table[entry_chunk_idx].entry_level_external_module_idx.is_empty()
          && !self.order_wrap_host_can_expose_then_export(
            chunk_graph,
            entry_chunk_idx,
            &importer_chunks,
          );

      if collapse_carries_trigger {
        let entry_module = self.link_output.module_table[entry_module_idx]
          .as_normal()
          .expect("dynamic entry should be a normal module");
        let namespace_exports = self.simulated_facade_namespace_exports(entry_module_idx);
        let bits = chunk_graph.chunk_table[entry_chunk_idx].bits.clone();
        let entry_chunk = &mut chunk_graph.chunk_table[entry_chunk_idx];
        entry_chunk.kind = ChunkKind::Common;
        entry_chunk.add_creation_reason(
          ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
          self.options,
        );
        chunk_graph
          .common_chunk_exported_facade_chunk_namespace
          .entry(entry_chunk_idx)
          .or_default()
          .insert(entry_module_idx);
        order_state.insert_simulated_facade_namespace(
          entry_module_idx,
          entry_module.namespace_object_ref,
          entry_chunk_idx,
          RuntimeHelper::ExportAll,
          namespace_exports,
          importer_sites.iter().map(|importer| importer.module_idx),
        );
        created = true;
        continue;
      }

      let Some((meta, bit, name, file_name, bits, input_base, preserve_entry_signature)) = ({
        let entry_chunk = &mut chunk_graph.chunk_table[entry_chunk_idx];
        match entry_chunk.kind {
          ChunkKind::EntryPoint { meta, bit, module }
            if module == entry_module_idx && !entry_chunk.modules.is_empty() =>
          {
            let bits = entry_chunk.bits.clone();
            let input_base = entry_chunk.input_base.clone();
            let name = entry_chunk.name.take();
            let file_name = entry_chunk.file_name.take();
            let preserve_entry_signature = entry_chunk.preserve_entry_signature.take();
            entry_chunk.kind = ChunkKind::Common;
            entry_chunk.add_creation_reason(
              ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
              self.options,
            );
            Some((meta, bit, name, file_name, bits, input_base, preserve_entry_signature))
          }
          ChunkKind::EntryPoint { .. } | ChunkKind::Common => None,
        }
      }) else {
        continue;
      };

      let mut facade_chunk = Chunk::new(
        name,
        file_name,
        bits,
        vec![],
        ChunkKind::EntryPoint { meta, bit, module: entry_module_idx },
        input_base,
        preserve_entry_signature,
      );
      let entry_module = &self.link_output.module_table[entry_module_idx];
      facade_chunk.add_creation_reason(
        ChunkCreationReason::Entry {
          is_user_defined_entry: meta.contains(ChunkMeta::UserDefinedEntry),
          entry_module_id: entry_module.stable_id(),
          name: self
            .link_output
            .entries
            .get(&entry_module_idx)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.name.as_ref()),
        },
        self.options,
      );
      let facade_chunk_idx = chunk_graph.add_chunk(facade_chunk);
      chunk_graph.entry_module_to_entry_chunk.insert(entry_module_idx, facade_chunk_idx);
      if let Some(reference_ids) = chunk_graph.chunk_idx_to_reference_ids.remove(&entry_chunk_idx) {
        chunk_graph.chunk_idx_to_reference_ids.insert(facade_chunk_idx, reference_ids);
      }
      created = true;
    }
    created
  }

  fn simulated_facade_namespace_exports(
    &self,
    entry_module_idx: ModuleIdx,
  ) -> Vec<SimulatedFacadeNamespaceExport> {
    self.link_output.metas[entry_module_idx]
      .referenced_canonical_exports_symbols(
        entry_module_idx,
        EntryPointKind::DynamicImport,
        &self.link_output.dynamic_import_exports_usage_map,
        false,
      )
      .map(|(name, export)| {
        let canonical_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
        let is_inlinable_constant =
          self.link_output.global_constant_symbol_map.get(&canonical_ref).is_some_and(|meta| {
            !meta.commonjs_export
              && (!self.options.optimization.is_inline_const_smart_mode() || meta.safe_to_inline)
          });
        SimulatedFacadeNamespaceExport {
          name: name.clone(),
          referenced_symbol: (!is_inlinable_constant).then_some(export.symbol_ref),
        }
      })
      .collect()
  }

  /// Collect every live `import()` call site for the requested dynamic entries. Statement
  /// inclusion is authoritative: a surviving import record in an excluded statement is not an
  /// emitted call site and therefore cannot carry an entry trigger.
  fn live_dynamic_importers(
    &self,
    chunk_graph: &ChunkGraph,
    targets: impl IntoIterator<Item = ModuleIdx>,
  ) -> FxHashMap<ModuleIdx, Vec<LiveDynamicImporter>> {
    let mut importers: FxHashMap<ModuleIdx, Vec<LiveDynamicImporter>> =
      targets.into_iter().map(|module_idx| (module_idx, Vec::new())).collect();
    for module in
      self.link_output.module_table.modules.iter().filter_map(|module| module.as_normal())
    {
      let meta = &self.link_output.metas[module.idx];
      if !meta.is_included {
        continue;
      }
      for (stmt_info_idx, stmt_info) in
        self.link_output.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt()
      {
        if !meta.stmt_info_included.has_bit(stmt_info_idx) {
          continue;
        }
        for rec_idx in &stmt_info.import_records {
          let rec = &module.import_records[*rec_idx];
          if rec.kind == ImportKind::DynamicImport
            && !rec.meta.contains(ImportRecordMeta::DeadDynamicImport)
            && let Some(importee_idx) = rec.resolved_module
            && let Some(target_importers) = importers.get_mut(&importee_idx)
          {
            target_importers.push(LiveDynamicImporter {
              module_idx: module.idx,
              trigger_chunk: chunk_graph.module_to_chunk[module.idx],
            });
          }
        }
      }
    }
    importers
  }

  fn restore_order_wrap_entry_facades(
    &self,
    chunk_graph: &mut ChunkGraph,
    plan: &OrderWrapPlan,
    order_state: &mut OrderWrapState,
  ) -> bool {
    if self.options.code_splitting.is_disabled() {
      return false;
    }

    let entries_to_restore = plan
      .modules()
      .filter(|module_idx| self.link_output.entries.contains_key(module_idx))
      .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order())
      .collect_vec();
    if entries_to_restore.is_empty() {
      return false;
    }

    // One statement-aware scan serves every restore candidate. See `live_dynamic_importers`.
    let dynamic_importers =
      self.live_dynamic_importers(chunk_graph, entries_to_restore.iter().copied());
    let mut recorded_simulated_namespace = false;

    for entry_module_idx in entries_to_restore {
      // Every live importer is a dynamic one, so every entry path into this module is an `import()`
      // that the finalizer can rewrite to carry the trigger: `Promise.resolve().then(() => (init_x(),
      // ns))` when the importer shares the host chunk, `import('./host.js').then(n => (n.init_x(),
      // n.namespace))` when it does not. Neither needs a file of its own, so the eliminated facade
      // stays eliminated. The trade this accepts: the trigger then runs in the rewrite's `.then`, one
      // microtask after the host chunk settles, instead of inside a facade's own evaluation — see
      // `m4_dynamic_facade_race`, which pins that ordering. A module with no live dynamic importer
      // has no call site to carry anything, so it keeps the facade, and an emitted chunk keeps its
      // facade because an `emitFile` reference id must resolve to a real file. See
      // internal-docs/code-splitting/design.md ("Trigger placement") for the policy this
      // implements.
      //
      // One shape breaks that rewrite and keeps the facade: the cross-chunk rewrite reads the
      // target's exports back out of the *host chunk's* namespace, which it obtains by resolving
      // `import('./host.js')`. A namespace carrying a callable `then` is assimilated as a thenable,
      // so the extraction callback never receives it — a chunk-mate's export would change what
      // `import()` of the target observes, which it cannot do in the source.
      // `order_wrap_host_can_expose_then_export` detects that.
      let entry_host_chunk = chunk_graph.module_to_chunk[entry_module_idx];
      let importer_sites = &dynamic_importers[&entry_module_idx];
      let importer_chunks =
        importer_sites.iter().map(|importer| importer.trigger_chunk).collect_vec();
      let collapse_carries_trigger = entry_host_chunk.is_some()
        && !importer_sites.is_empty()
        && importer_sites.iter().all(|importer| importer.trigger_chunk.is_some())
        && !entry_host_chunk.is_some_and(|host_chunk_idx| {
          self.order_wrap_host_can_expose_then_export(chunk_graph, host_chunk_idx, &importer_chunks)
        });

      let eliminated_facade_chunk_indices = chunk_graph
        .chunk_table
        .iter_enumerated()
        .filter_map(|(chunk_idx, chunk)| match chunk.kind {
          ChunkKind::EntryPoint { meta, module, .. }
            if module == entry_module_idx
              && chunk.modules.is_empty()
              && meta.intersects(ChunkMeta::DynamicImported | ChunkMeta::EmittedChunk)
              && matches!(
                chunk_graph.post_chunk_optimization_operations.get(&chunk_idx),
                Some(
                  PostChunkOptimizationOperation::Removed
                    | PostChunkOptimizationOperation::RemovedWithPreservedExports
                )
              ) =>
          {
            Some(chunk_idx)
          }
          ChunkKind::EntryPoint { .. } | ChunkKind::Common => None,
        })
        .collect_vec();
      if eliminated_facade_chunk_indices.is_empty() {
        continue;
      }
      let facade_chunk_indices = eliminated_facade_chunk_indices
        .into_iter()
        .filter(|chunk_idx| {
          matches!(
            chunk_graph.chunk_table[*chunk_idx].kind,
            ChunkKind::EntryPoint { meta, .. }
              if meta.contains(ChunkMeta::EmittedChunk) || !collapse_carries_trigger
          )
        })
        .collect_vec();
      if facade_chunk_indices.is_empty() {
        debug_assert!(collapse_carries_trigger);
        let entry_host_chunk = entry_host_chunk.expect("collapsed entry should have a host chunk");
        debug_assert!(
          chunk_graph
            .common_chunk_exported_facade_chunk_namespace
            .get(&entry_host_chunk)
            .is_some_and(|entries| entries.contains(&entry_module_idx)),
          "optimizer-eliminated dynamic facade should publish its namespace from the host chunk",
        );
        let entry_module = self.link_output.module_table[entry_module_idx]
          .as_normal()
          .expect("dynamic entry should be a normal module");
        order_state.insert_simulated_facade_namespace(
          entry_module_idx,
          entry_module.namespace_object_ref,
          entry_host_chunk,
          RuntimeHelper::ExportAll,
          self.simulated_facade_namespace_exports(entry_module_idx),
          importer_sites.iter().map(|importer| importer.module_idx),
        );
        recorded_simulated_namespace = true;
        continue;
      }
      let Some(&facade_chunk_idx) = facade_chunk_indices.first() else {
        unreachable!("empty facade list handled above");
      };

      if let Some(current_chunk_idx) =
        chunk_graph.entry_module_to_entry_chunk.insert(entry_module_idx, facade_chunk_idx)
      {
        let should_remove_key = if let Some(set) =
          chunk_graph.common_chunk_exported_facade_chunk_namespace.get_mut(&current_chunk_idx)
        {
          set.remove(&entry_module_idx);
          set.is_empty()
        } else {
          false
        };
        if should_remove_key {
          chunk_graph.common_chunk_exported_facade_chunk_namespace.remove(&current_chunk_idx);
        }
      }

      for facade_chunk_idx in facade_chunk_indices {
        chunk_graph.post_chunk_optimization_operations.remove(&facade_chunk_idx);
      }
    }
    recorded_simulated_namespace
  }

  /// Whether collapsing a dynamic entry into `host_chunk_idx` could let a cross-chunk `import()`
  /// of that chunk resolve to something other than the chunk's namespace. See
  /// internal-docs/code-splitting/design.md ("Trigger placement") for the trigger-siting policy
  /// this guards.
  ///
  /// The cross-chunk rewrite is `import('./host.js').then(n => (n.init_x(), n.namespace))`, so the
  /// host chunk's namespace passes through the promise resolution procedure. If that namespace
  /// exposes a callable `then`, it is assimilated as a thenable and `n` becomes whatever that
  /// `then` resolves with — the extraction callback never sees the namespace. In the source the
  /// target's `import()` cannot observe a sibling module's exports at all, so this is a behaviour
  /// change and the facade (whose namespace holds only the entry's own exports) has to stay.
  ///
  /// Same-chunk importers are immune: their rewrite is `Promise.resolve().then(() => ...)` and
  /// never dynamically imports the host, so the guard only applies once some importer sits
  /// elsewhere. The check is deliberately over-approximate — every module placed in the chunk
  /// counts, not just the ones whose `then` actually reaches the chunk's export list — because a
  /// false positive only restores a facade while a false negative breaks user code.
  ///
  /// Both name spaces have to be inspected, because `deconflict_exported_names` picks the emitted
  /// name from a different one per chunk kind: an entry chunk exports its entry module's canonical
  /// exports under their source-level alias, while everything reaching a chunk through the
  /// cross-chunk symbol path is emitted under the *declaring symbol's* name. So `export { then as
  /// hostThen }` is a hazard even though no export is named `then`, and `export { local as then }`
  /// is a hazard even though no symbol is named `then`.
  fn order_wrap_host_can_expose_then_export(
    &self,
    chunk_graph: &ChunkGraph,
    host_chunk_idx: ChunkIdx,
    importer_chunks: &[Option<ChunkIdx>],
  ) -> bool {
    if importer_chunks.iter().all(|importer_chunk| *importer_chunk == Some(host_chunk_idx)) {
      return false;
    }

    let symbol_db = &self.link_output.symbol_db;
    let module_can_expose_then = |module_idx: ModuleIdx| {
      self.link_output.metas[module_idx].resolved_exports.iter().any(|(name, export)| {
        name.as_str() == THEN
          || symbol_db.canonical_ref_for(export.symbol_ref).name(symbol_db) == THEN
      })
    };

    let host_chunk = &chunk_graph.chunk_table[host_chunk_idx];
    // An entry chunk keeps its entry module's own export names even when internal exports are
    // minified (see `deconflict_exported_names`), so those always reach the emitted namespace.
    if let ChunkKind::EntryPoint { module, .. } = host_chunk.kind
      && module_can_expose_then(module)
    {
      return true;
    }

    // Mirrors `deconflict_exported_names`: when internal export names are minified they become
    // short generated identifiers that can never be `then`, so only the modules explicitly held
    // back from minification can still contribute one.
    if !self.options.preserve_modules && self.options.minify_internal_exports {
      return chunk_graph
        .common_chunk_preserve_export_names_modules
        .get(&host_chunk_idx)
        .is_some_and(|modules| modules.iter().copied().any(module_can_expose_then));
    }

    host_chunk.modules.iter().copied().any(module_can_expose_then)
  }

  /// Normalize the runtime module onto a standalone chunk. Returns whether the caller still owes
  /// the runtime-chunk merge re-proof (`fold_runtime_chunk_after_order_lowering`).
  ///
  /// The baseline `try_merge_runtime_chunk` calls run before order analysis, so a pre-lowering
  /// merge never proved anything about the helper demand the wrappers and overlays above added.
  /// Evicting a co-hosted runtime first restores the standalone shape that proof requires; by this
  /// point lowering has materialized every order-introduced demand in [`OrderWrapState`], so the
  /// re-proof sees the complete consumer set.
  ///
  /// Normalizing has to happen before the entry-facade edge query — resolving a depended symbol to
  /// its chunk needs the runtime module to sit in one — while the re-proof has to happen after it,
  /// because it counts the runtime chunk's consumers and facade creation is what settles them. So
  /// the two are split: this returns the obligation and `apply_order_wraps` discharges it once the
  /// facade topology is final.
  fn ensure_runtime_module_for_order_wraps(&mut self, chunk_graph: &mut ChunkGraph) -> bool {
    let runtime_idx = self.link_output.runtime.id();
    if let Some(runtime_chunk_idx) = chunk_graph.module_to_chunk[runtime_idx] {
      if self.options.code_splitting.is_disabled() {
        return false;
      }
      let runtime_chunk = &chunk_graph.chunk_table[runtime_chunk_idx];
      if runtime_chunk.modules.len() == 1 {
        self.clear_module_symbol_chunk_indices(runtime_idx);
        // Facade restoration can tombstone chunks the pre-lowering merge counted as consumers, so
        // a standalone runtime may only now have a sole consumer left.
        return true;
      }
      let mut bits = runtime_chunk.bits.clone();
      for chunk_idx in self.live_chunks(chunk_graph) {
        bits.union(&chunk_graph.chunk_table[chunk_idx].bits);
      }
      let input_base = runtime_chunk.input_base.clone();
      chunk_graph.chunk_table[runtime_chunk_idx]
        .modules
        .retain(|module_idx| *module_idx != runtime_idx);
      self.update_chunk_runtime_helpers_after_module_removal(
        chunk_graph,
        runtime_chunk_idx,
        runtime_idx,
      );
      let mut new_runtime_chunk = Chunk::new(
        Some("rolldown-runtime".into()),
        None,
        bits,
        vec![],
        ChunkKind::Common,
        input_base,
        None,
      );
      let runtime_chunk_bits = new_runtime_chunk.bits.clone();
      new_runtime_chunk.add_creation_reason(
        ChunkCreationReason::CommonChunk {
          bits: &runtime_chunk_bits,
          link_output: self.link_output,
        },
        self.options,
      );
      let new_runtime_chunk_idx = chunk_graph.add_chunk(new_runtime_chunk);
      chunk_graph.add_module_to_chunk(
        runtime_idx,
        new_runtime_chunk_idx,
        self.link_output.metas[runtime_idx].depended_runtime_helper,
      );
      self.clear_module_symbol_chunk_indices(runtime_idx);
      return true;
    }

    let live_chunk_indices = self.live_chunks(chunk_graph);
    let Some(first_chunk_idx) = live_chunk_indices.first().copied() else {
      return false;
    };

    if self.options.code_splitting.is_disabled() || live_chunk_indices.len() == 1 {
      let chunk = &mut chunk_graph.chunk_table[first_chunk_idx];
      chunk.modules.insert(0, runtime_idx);
      chunk_graph.module_to_chunk[runtime_idx] = Some(first_chunk_idx);
      self.clear_module_symbol_chunk_indices(runtime_idx);
      return false;
    }

    let mut bits = chunk_graph.chunk_table[first_chunk_idx].bits.clone();
    for chunk_idx in live_chunk_indices.iter().copied().skip(1) {
      bits.union(&chunk_graph.chunk_table[chunk_idx].bits);
    }

    let input_base = chunk_graph.chunk_table[first_chunk_idx].input_base.clone();
    let mut runtime_chunk = Chunk::new(
      Some("rolldown-runtime".into()),
      None,
      bits,
      vec![],
      ChunkKind::Common,
      input_base,
      None,
    );
    let runtime_chunk_bits = runtime_chunk.bits.clone();
    runtime_chunk.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &runtime_chunk_bits, link_output: self.link_output },
      self.options,
    );
    let runtime_chunk_idx = chunk_graph.add_chunk(runtime_chunk);
    chunk_graph.add_module_to_chunk(
      runtime_idx,
      runtime_chunk_idx,
      self.link_output.metas[runtime_idx].depended_runtime_helper,
    );
    self.clear_module_symbol_chunk_indices(runtime_idx);
    true
  }

  /// Re-run the runtime-chunk merge proof against the post-lowering consumer set: the
  /// order-introduced consumers from [`OrderWrapState`] plus the merge's own re-scan of every
  /// pre-lowering consumer. Restricted to a sole-consumer host — see
  /// [`RuntimeMergeCascade::SingleConsumerOnly`].
  ///
  /// Esm output only. Under cjs output, `compute_cross_chunk_links` later gives every ESM-exports
  /// entry chunk — including the zero-module facades minted above — a `__toCommonJS` demand that
  /// is invisible here, so a fold could hand an entry chunk with no demand at all a brand-new
  /// require edge into a user chunk. Other formats keep the standalone/evicted layout.
  fn fold_runtime_chunk_after_order_lowering(
    &self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
  ) {
    if !matches!(self.options.format, OutputFormat::Esm) {
      return;
    }
    let order_consumers = order_state.runtime_helper_consumer_chunks(&chunk_graph.module_to_chunk);
    self.try_merge_runtime_chunk(
      chunk_graph,
      Some(&order_consumers),
      RuntimeMergeCascade::SingleConsumerOnly,
    );
  }

  fn update_chunk_runtime_helpers_after_module_removal(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
    removed_module_idx: ModuleIdx,
  ) {
    let mut helpers = chunk_graph.chunk_table[chunk_idx].depended_runtime_helper;
    helpers.remove(self.link_output.metas[removed_module_idx].depended_runtime_helper);
    helpers.insert(
      chunk_graph.chunk_table[chunk_idx]
        .modules
        .iter()
        .fold(RuntimeHelper::default(), |helpers, module_idx| {
          helpers | self.link_output.metas[*module_idx].depended_runtime_helper
        }),
    );
    chunk_graph.chunk_table[chunk_idx].depended_runtime_helper = helpers;
  }

  fn clear_module_symbol_chunk_indices(&mut self, module_idx: ModuleIdx) {
    let Some(local_db) = self.link_output.symbol_db[module_idx].as_mut() else {
      return;
    };
    for symbol_data in &mut local_db.classic_data {
      symbol_data.chunk_idx = None;
    }
  }

  fn live_chunks(&self, chunk_graph: &ChunkGraph) -> Vec<ChunkIdx> {
    chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(chunk_idx, _)| chunk_graph.chunk_is_live(chunk_idx).then_some(chunk_idx))
      .collect_vec()
  }

  fn renumber_live_chunks(&self, chunk_graph: &mut ChunkGraph) {
    let live_chunks = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, _)| chunk_graph.chunk_is_live(*chunk_idx))
      .sorted_by_key(|(chunk_idx, chunk)| (chunk.exec_order, chunk_idx.raw()))
      .map(|(chunk_idx, _)| chunk_idx)
      .collect_vec();

    for (exec_order, chunk_idx) in live_chunks.iter().copied().enumerate() {
      chunk_graph.chunk_table[chunk_idx].exec_order =
        exec_order.try_into().expect("Too many chunks, u32 overflowed.");
    }

    chunk_graph.rebuild_sorted_chunk_idx_vec(true);
  }

  pub(super) fn esm_runtime_helper(&self) -> RuntimeHelper {
    if self.options.profiler_names { RuntimeHelper::Esm } else { RuntimeHelper::EsmMin }
  }
}

fn lower_order_state(
  input: &OrderLoweringInput<'_>,
  output: &mut OrderLoweringOutput<'_>,
  runtime_helper: RuntimeHelper,
  code_splitting_disabled: bool,
) {
  for module_idx in
    input.plan.modules().sorted_unstable_by_key(|idx| input.modules[*idx].exec_order())
  {
    if !matches!(input.linking[module_idx].wrap_kind(), WrapKind::None) {
      continue;
    }
    let module =
      input.modules[module_idx].as_normal().expect("order wrap only applies to normal modules");
    let wrapper_ref = output
      .symbols
      .create_facade_root_symbol_ref(module_idx, &format!("init_{}", module.repr_name));
    output.state.insert_order_wrapper(module_idx, wrapper_ref, runtime_helper);
    if order_wrapper_is_reexport_transparent(
      &input.linking[module_idx],
      input.asts[module_idx].as_ref(),
      input.keep_names,
    ) {
      output.state.set_reexport_init_transparent(module_idx);
    }
  }

  let consumer_local_plan = consumer_local_reexport_plan(input, output.state);
  apply_consumer_local_reexport_plan(input, output, runtime_helper, &consumer_local_plan);

  let reexport_usage = collect_frozen_reexport_usage(input, output.state);
  output.state.set_nested_reexport_records(reexport_usage.nested_records.clone());
  output.state.set_consumed_reexport_facades(reexport_usage.consumed_facades.clone());

  // Real lowering runs once per bundle, so it builds its own reverse index here; the fixpoint
  // projector passes the analysis-owned one instead of rebuilding per round.
  let reverse_static_imports = super::order_analysis::reverse_static_import_index(input.modules);
  populate_order_import_overlays(
    input,
    &reexport_usage,
    output.state,
    code_splitting_disabled,
    &reverse_static_imports,
  );
}

/// Mint the per-record [`OrderImportOverlay`]s for the current plan: a wrapper-referencing overlay
/// for a re-export/execution-dependency import of a planned direct target, and a
/// retained-re-export-path overlay for a re-export that itself reaches the plan through a
/// tree-shaken barrel. Split out of [`lower_order_state`] so the emergent-cycle fixpoint projector
/// can populate an identical set of overlays on its probe state — the overlays and the nested
/// re-export records are what let the final metadata pass's `transitive_esm_init_targets` restrict
/// a barrel's hop walk to its retained path, so projection stays byte-faithful to the real
/// registration instead of over-approximating. Reads and writes only the [`OrderWrapState`]; it
/// never mints symbols, so the projector can drive it with each module's namespace ref as a wrapper
/// placeholder.
pub(super) fn populate_order_import_overlays(
  input: &OrderLoweringInput<'_>,
  reexport_usage: &FrozenReexportUsage,
  state: &mut OrderWrapState,
  code_splitting_disabled: bool,
  reverse_static_imports: &oxc_index::IndexVec<ModuleIdx, Vec<ModuleIdx>>,
) {
  // Backward closure of the plan over the reverse static-import index: one walk answers every
  // record's "does this importee's static-import subtree reach a plan member" instead of a
  // per-record DFS.
  let mut reaches_plan = FxHashSet::default();
  super::order_analysis::grow_static_import_backward_closure(
    reverse_static_imports,
    input.plan.modules(),
    &mut reaches_plan,
  );
  for (importer_idx, module) in input.modules.iter_enumerated() {
    let Some(importer) = module.as_normal() else {
      continue;
    };
    if state.is_consumer_local_reexport_route(importer_idx) {
      continue;
    }
    let execution_dependencies = &input.linking[importer_idx].execution_dependencies;
    for (stmt_info_idx, stmt_info) in input.statements[importer_idx].iter_enumerated() {
      for &rec_idx in &stmt_info.import_records {
        let rec = &importer.import_records[rec_idx];
        let Some(importee_idx) = rec.resolved_module else {
          continue;
        };
        let direct_target_is_planned = input.plan.contains(&importee_idx);
        let retained_reexport_path =
          retained_order_reexport_path(input, reexport_usage, importer_idx, stmt_info_idx, rec_idx);
        if !execution_dependencies.contains(&importee_idx) && retained_reexport_path.is_none() {
          continue;
        }
        let Some(importee) = input.modules[importee_idx].as_normal() else {
          continue;
        };
        if !direct_target_is_planned {
          if let Some(retained_reexport_path) = retained_reexport_path
            && reaches_plan.contains(&importee_idx)
          {
            state.insert_import_overlay(
              OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
              OrderImportOverlay::transitive_reexport(retained_reexport_path),
              importer.namespace_object_ref,
              importee.namespace_object_ref,
            );
          }
          continue;
        }
        let Some(init_target) = state.esm_init_target(importee_idx, &input.linking[importee_idx])
        else {
          continue;
        };
        let mut overlay = OrderImportOverlay::from_import_record(
          rec.kind,
          rec.meta,
          init_target.wrapper_ref,
          importer.namespace_object_ref,
          importee.namespace_object_ref,
          input.linking[importee_idx].has_dynamic_exports,
          execution_dependencies.contains(&importee_idx),
          code_splitting_disabled,
        );
        if let Some(overlay) = &mut overlay
          && let Some(retained_reexport_path) = retained_reexport_path
        {
          overlay.retained_reexport_path = retained_reexport_path;
          // A retained-path statement is emitted entirely from the shared target resolver. The
          // overlay still owns any namespace/runtime glue, but registering its direct wrapper as
          // well would create a cross-chunk import that finalization never calls — and can pull a
          // carrier-hosting barrel chunk into an unrelated consumer's static closure.
          if !overlay.retained_reexport_path.is_empty() {
            overlay.referenced_symbols.retain(|symbol_ref| *symbol_ref != init_target.wrapper_ref);
          }
        }
        if let Some(overlay) = overlay {
          state.insert_import_overlay(
            OrderImportKey { importer: importer_idx, statement: stmt_info_idx, record: rec_idx },
            overlay,
            importer.namespace_object_ref,
            importee.namespace_object_ref,
          );
        }
      }
    }
  }
}

pub(super) fn collect_frozen_reexport_usage(
  input: &OrderLoweringInput<'_>,
  state: &OrderWrapState,
) -> FrozenReexportUsage {
  let mut consumed_facades = FxHashSet::default();
  for (used_ref, chain) in input.export_chains {
    if input.used_symbols.contains(used_ref) {
      consumed_facades.extend(chain.iter().copied());
    }
  }

  let mut root_paths =
    FxHashMap::<(ModuleIdx, ImportRecordIdx), Vec<(ModuleIdx, ImportRecordIdx)>>::default();
  for (imported_as_ref, paths) in input.star_reexport_records_by_imported_symbol {
    // A namespace-keyed path (recorded for a whole consumed namespace by
    // `record_namespace_consumed_star_reexport_paths`, or a member read resolving to a
    // namespace-valued binding) is consumed exactly when that namespace object is materialized:
    // an included namespace retains every non-ambiguous export, so its star chains are
    // execution-relevant; symbol-level usedness would conflate routes (a leaf used through a
    // direct import elsewhere must not retain a barrel path nobody consumes).
    let key_is_namespace = input.modules[imported_as_ref.owner]
      .as_normal()
      .is_some_and(|module| module.namespace_object_ref == *imported_as_ref);
    for path in paths {
      let Some(root) = path.first().copied() else {
        continue;
      };
      let consumer_is_used = if key_is_namespace {
        // `namespace_included` here is the provisional pre-wrap value: `finalize_chunk_plan` runs
        // `finalized_module_namespace_ref_usage` before order analysis/lowering and re-runs it
        // only after. The skew is safe — the post-wrap refinement can only ADD namespaces
        // demanded by import overlays (`requires_namespace`: `export *` of a dynamic-exports
        // importee, `require` interop, splitting-disabled dynamic import), and those routes
        // discharge their breadth at runtime through `__reExport`/`__toCommonJS` glue rather
        // than statically routed init forwarding. An opaque `import * as` consumer — the demand
        // this gate exists for — is a link-time fact the provisional pass already observes.
        input.linking[imported_as_ref.owner].namespace_included
      } else {
        input.used_symbols.contains(imported_as_ref)
          || consumed_facades.contains(imported_as_ref)
          || input.linking[root.0]
            .referenced_symbols_by_entry_point_chunk
            .iter()
            .any(|(symbol_ref, _)| symbol_ref == imported_as_ref)
      };
      if consumer_is_used {
        root_paths.entry(root).or_default().extend(path.iter().copied());
        // An ancestor's excluded-hop traversal stops at the first init-owning barrel it meets and
        // delegates the rest of the chain to that barrel's own `init_*`
        // (`collect_order_wrap_esm_init_targets` pushes the owning wrapper without descending).
        // That delegation is only sound if the owning barrel itself carries the remainder as
        // retained evidence, so record each such suffix as that barrel's own root — otherwise its
        // interior hop forwards nothing and the chain's pure leaf is never initialized.
        for (position, record) in path.iter().copied().enumerate().skip(1) {
          if module_owns_reexport_init(input, state, record.0) {
            root_paths.entry(record).or_default().extend(path[position..].iter().copied());
          }
        }
      }
    }
  }

  let mut nested_records = FxHashSet::default();
  for (root, path) in &mut root_paths {
    path.sort_unstable_by_key(|(module_idx, rec_idx)| (module_idx.index(), rec_idx.index()));
    path.dedup();
    // A record is "nested" only when a wrapped ancestor barrel's traversal walks *through* its
    // importer to reach a deeper wrapped target, so the ancestor already owns that init and the
    // interior record must stay silent. That traversal stops at the first non-transparent wrapped
    // barrel it meets, delegating the rest of the chain to that barrel's own `init_*`. A
    // transparent order wrapper remains a waypoint instead: making it own the hop would let an
    // unrelated consumer of the shared barrel initialize retained leaves too early.
    nested_records.extend(
      path
        .iter()
        .copied()
        .filter(|record| record != root)
        .filter(|(module_idx, _)| !module_owns_reexport_init(input, state, *module_idx)),
    );
  }

  FrozenReexportUsage { root_paths, nested_records, consumed_facades }
}

/// Whether `module_idx` owns re-export initialization: an interop `WrapKind::Esm` wrapper or a
/// non-transparent order wrapper selected by the plan. A transparent order wrapper has no local
/// executable body or unconditional execution dependency, so retained paths cross it and stay
/// owned by the consuming ancestor instead of becoming shared barrel-wide work.
///
/// Concatenated wrapped modules — which would share their group's init rather than own a standalone
/// one — are not supported on this branch (order wrapping never marks a module
/// `ConcatenateWrappedModuleKind::Inner`/`Root`), so no concatenated-kind guard is needed here.
/// Re-add one if concatenated-wrapper support lands.
fn module_owns_reexport_init(
  input: &OrderLoweringInput<'_>,
  state: &OrderWrapState,
  module_idx: ModuleIdx,
) -> bool {
  (matches!(input.linking[module_idx].wrap_kind(), WrapKind::Esm)
    || input.plan.contains(&module_idx))
    && !state.reexport_init_is_transparent(module_idx)
}

fn retained_order_reexport_path(
  input: &OrderLoweringInput<'_>,
  reexport_usage: &FrozenReexportUsage,
  importer_idx: ModuleIdx,
  stmt_info_idx: StmtInfoIdx,
  rec_idx: ImportRecordIdx,
) -> Option<Vec<(ModuleIdx, ImportRecordIdx)>> {
  let importer = input.modules[importer_idx].as_normal()?;
  let rec = &importer.import_records[rec_idx];
  if !rec.meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly) {
    return None;
  }
  let meta = &input.linking[importer_idx];
  if let Some(path) = reexport_usage.root_paths.get(&(importer_idx, rec_idx)) {
    return Some(path.clone());
  }
  if reexport_usage.nested_records.contains(&(importer_idx, rec_idx)) {
    return None;
  }
  if meta.stmt_info_included.has_bit(stmt_info_idx) {
    return Some(vec![]);
  }
  if rec.meta.contains(ImportRecordMeta::IsExportStar)
    && meta.namespace_included
    && rec
      .resolved_module
      .is_some_and(|importee_idx| input.linking[importee_idx].has_dynamic_exports)
  {
    return Some(vec![]);
  }

  let facade_is_retained = |facade_ref: SymbolRef| {
    input.used_symbols.contains(&facade_ref)
      || reexport_usage.consumed_facades.contains(&facade_ref)
  };
  (importer
    .named_imports
    .iter()
    .any(|(facade_ref, import)| import.record_idx == rec_idx && facade_is_retained(*facade_ref))
    || input.statements[importer_idx][stmt_info_idx]
      .declared_symbols
      .iter()
      .any(|declared| facade_is_retained(declared.inner())))
  .then_some(vec![])
}
