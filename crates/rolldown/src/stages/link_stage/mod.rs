use arcstr::ArcStr;
use itertools::Itertools;
use nodejs_built_in_modules::is_nodejs_builtin_module;
use oxc::span::Span;
use oxc_index::IndexVec;
use petgraph::{
  algo::dominators::simple_fast,
  graph::{DiGraph, NodeIndex},
};
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  ConstExportMeta, DependedRuntimeHelperMap, EcmaModuleAstUsage, EntryPoint, EntryPointKind,
  FlatOptions, ImportKind, IndexModules, ModuleIdx, ModuleTable, PreserveEntrySignatures,
  RetainedExportSymbols, RuntimeModuleBrief, StmtInfoIdx, SymbolRef, SymbolRefDb,
  UsedExternalSymbols, UsedSymbolRefsBuilder, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::Diagnostics;
#[cfg(target_family = "wasm")]
use rolldown_utils::rayon::IteratorExt as _;
use rolldown_utils::{
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  stages::link_stage::tree_shaking::determine_side_effects::ScanTimeSideEffects,
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    member_read_star_reexport_path::MemberReadStarReexportPath,
  },
};

use super::scan_stage::NormalizedScanStageOutput;

mod bind_imports_and_exports;
mod compute_tla;
mod create_exports_for_ecma_modules;
mod cross_module_optimization;
mod determine_module_exports_kind;
mod generate_lazy_export;
mod patch_module_dependencies;
mod promote_deferred_write_constants;
mod prune_constant_branches;
mod reference_needed_symbols;
mod refine_stmt_side_effects;
mod sort_modules;
mod tree_shaking;

pub use tree_shaking::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol, include_symbol,
};
mod wrapping;

/// Information about safely merged CJS namespaces for a module
#[derive(Debug, Default, Clone)]
pub struct SafelyMergeCjsNsInfo {
  /// Namespace symbol refs that can be merged into a single binding
  pub namespace_refs: Vec<SymbolRef>,
  /// Whether this CJS module needs `__toESM` interop (has namespace or default imports)
  pub needs_interop: bool,
}

#[derive(Debug)]
pub struct LinkStageOutput {
  pub module_table: ModuleTable,
  pub entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  /// Per-module statement-info table; see `LinkStage.stmt_infos`.
  pub stmt_infos: IndexStmtInfos,
  pub runtime: RuntimeModuleBrief,
  pub diagnostics: Diagnostics,
  pub used_external_symbols: UsedExternalSymbols,
  /// See [`RetainedExportSymbols`]; empty until the generate stage projects it.
  pub retained_export_symbols: RetainedExportSymbols,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  /// https://rollupjs.org/plugin-development/#this-emitfile
  /// Used to store `preserveSignature` specified with `this.emitFile` in plugins.
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub star_reexport_records_by_imported_symbol:
    FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, rolldown_common::ImportRecordIdx)>>>,
  pub member_read_star_reexport_paths: Vec<MemberReadStarReexportPath>,
  /// See `IncludeContext::kept_inlined_constant_reads`.
  pub kept_inlined_constant_reads: FxHashSet<SymbolRef>,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// True if any module has enum member values to inline. Computed once to avoid
  /// repeated full module table scans.
  pub has_enum_inlining: bool,
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  pub symbols: SymbolRefDb,
  /// Per-module runtime-helper-dependency map. Detached from `EcmaView` so the
  /// parallel walk in `reference_needed_symbols` can mutate it through `&mut`
  /// from a zipped iterator without aliasing tricks.
  pub depended_runtime_helper: IndexVec<ModuleIdx, Box<DependedRuntimeHelperMap>>,
  /// Per-module statement-info table. Detached from `EcmaView` at `LinkStage::new`
  /// (the field on `EcmaView` is left as an empty placeholder) so the parallel
  /// walk in `reference_needed_symbols` can mutate it through `&mut` from a
  /// zipped iterator without aliasing tricks. Threaded through `LinkStageOutput`
  /// to the generate stage and module finalizers, which used to read
  /// `module.stmt_infos` directly.
  pub stmt_infos: IndexStmtInfos,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub diagnostics: Diagnostics,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs_builder: UsedSymbolRefsBuilder,
  pub used_external_symbols: UsedExternalSymbols,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub star_reexport_records_by_imported_symbol:
    FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, rolldown_common::ImportRecordIdx)>>>,
  pub member_read_star_reexport_paths: Vec<MemberReadStarReexportPath>,
  pub kept_inlined_constant_reads: FxHashSet<SymbolRef>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  /// Imports whose binding lazy barrel left uncertain by skipping a record they depend on.
  pub lazy_barrel_uncertain_bindings: Vec<SymbolRef>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  /// Centralized map of modules that use top-level `await` → span of the
  /// first TLA keyword. Populated during the scan stage.
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
  /// Computed during `include_statements`, reused when building `LinkStageOutput`.
  pub has_enum_inlining: bool,
  /// The writer statements of each constant that `promote_deferred_write_constants` added.
  promoted_constant_writers: Vec<(ModuleIdx, Vec<StmtInfoIdx>)>,
  /// Module side-effect verdicts as the scan stage left them. See
  /// `recompute_analyzed_side_effects`.
  pub scan_time_side_effects: IndexVec<ModuleIdx, ScanTimeSideEffects>,
  /// Modules in which a link pass relaxed statement flags after `determine_side_effects` ran.
  pub relaxed_side_effect_modules: FxHashSet<ModuleIdx>,
  /// Lazily computed static-import components for entry-independent constant lookups.
  import_cycles: refine_stmt_side_effects::ImportCycles,
}

impl<'a> LinkStage<'a> {
  pub fn new(mut scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
    scan_stage_output.symbol_ref_db.begin_link();
    // since constant export is spared in most of time, aggregate them would make searching more efficient
    let constant_symbol_map = if options.optimization.is_inline_const_enabled() {
      scan_stage_output
        .module_table
        .modules
        .par_iter_mut()
        .filter_map(|m| {
          let m = m.as_normal_mut()?;
          Some(std::mem::take(&mut m.constant_export_map).into_iter().map(|(symbol_id, v)| {
            let symbol_ref = SymbolRef { owner: m.idx, symbol: symbol_id };
            (symbol_ref, v)
          }))
        })
        .flatten_iter()
        .collect::<FxHashMap<SymbolRef, ConstExportMeta>>()
    } else {
      FxHashMap::default()
    };

    Self::sort_entry_points(
      &mut scan_stage_output.entry_points,
      &scan_stage_output.module_table.modules,
    );

    let scan_time_side_effects = scan_stage_output
      .module_table
      .modules
      .iter()
      .map(|module| ScanTimeSideEffects {
        side_effects: *module.side_effects(),
        stmt_info_count: scan_stage_output.stmt_infos[module.idx()].len(),
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    Self {
      sorted_modules: Vec::new(),
      scan_time_side_effects,
      relaxed_side_effect_modules: FxHashSet::default(),
      import_cycles: refine_stmt_side_effects::ImportCycles::default(),
      global_constant_symbol_map: constant_symbol_map,
      depended_runtime_helper: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|_| Box::default())
        .collect::<IndexVec<ModuleIdx, _>>(),
      // `stmt_infos` is produced by the scan stage on the side (in
      // `NormalizedScanStageOutput.stmt_infos`) rather than living on each
      // `EcmaView`, so we can move it directly here.
      stmt_infos: std::mem::take(&mut scan_stage_output.stmt_infos),
      metas: scan_stage_output
        .module_table
        .modules
        .iter()
        .map(|module| {
          let mut meta = LinkingMetadata::default();
          meta.dependencies = module
            .import_records()
            .iter()
            .filter_map(|rec| match rec.kind {
              // Dynamically imported modules are included automatically by `include_statements`
              // when code splitting is disabled (`codeSplitting: false`).
              //
              // HotAccept is an HMR-only edge that should be filtered out here.
              // NewUrl needs at most the emitted asset's filename; it never executes the target
              // module. See internal-docs/plugin-asset-module/implementation.md.
              ImportKind::DynamicImport
              | ImportKind::Require
              | ImportKind::HotAccept
              | ImportKind::NewUrl => None,
              _ => rec.resolved_module,
            })
            .collect();
          meta.star_exports_from_external_modules = module.as_normal().map_or(vec![], |inner| {
            inner
              .star_exports_from_external_modules(&scan_stage_output.module_table.modules)
              .collect()
          });
          meta
        })
        .collect::<IndexVec<ModuleIdx, _>>(),
      module_table: scan_stage_output.module_table,
      entries: {
        let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
        for entry in scan_stage_output.entry_points {
          entries.entry(entry.idx).or_default().push(entry);
        }
        entries
      },
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      diagnostics: scan_stage_output.warnings.into(),
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      used_symbol_refs_builder: UsedSymbolRefsBuilder::default(),
      used_external_symbols: UsedExternalSymbols::default(),
      safely_merge_cjs_ns_map: FxHashMap::default(),
      normal_symbol_exports_chain_map: FxHashMap::default(),
      star_reexport_records_by_imported_symbol: FxHashMap::default(),
      member_read_star_reexport_paths: Vec::new(),
      kept_inlined_constant_reads: FxHashSet::default(),
      lazy_barrel_uncertain_bindings: Vec::new(),
      external_import_namespace_merger: FxHashMap::default(),
      overrode_preserve_entry_signature_map: scan_stage_output
        .overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: scan_stage_output.entry_point_to_reference_ids,
      flat_options: scan_stage_output.flat_options,
      user_defined_entry_modules: scan_stage_output.user_defined_entry_modules,
      tla_module_count: scan_stage_output.tla_module_count,
      tla_keyword_span_map: scan_stage_output.tla_keyword_span_map,
      has_enum_inlining: false,
      promoted_constant_writers: Vec::new(),
    }
  }

  fn sort_entry_points(entry_points: &mut Vec<EntryPoint>, modules: &IndexModules) {
    // Preserve declared input order.
    let mut rest = entry_points
      .extract_if(0.., |item| !matches!(item.kind, EntryPointKind::UserDefined))
      .collect_vec();

    rest.sort_by_cached_key(|entry| (entry.kind, modules[entry.idx].id().as_str()));
    let entry_order: FxIndexSet<ModuleIdx> = rest.iter().map(|entry| entry.idx).collect();
    if entry_order.len() > 1 {
      // See internal-docs/linking/module-execution-order/implementation.md.
      let mut graph = DiGraph::<(), ()>::with_capacity(modules.len() + 1, 0);
      for _ in modules {
        graph.add_node(());
      }
      let root = graph.add_node(());
      let mut static_importers: IndexVec<ModuleIdx, Vec<ModuleIdx>> =
        modules.iter().map(|_| Vec::new()).collect();
      for module in modules {
        for record in module.import_records() {
          if let Some(dependency) = record.resolved_module {
            graph.add_edge(
              NodeIndex::new(module.idx().index()),
              NodeIndex::new(dependency.index()),
              (),
            );
            if record.kind.is_static() {
              static_importers[dependency].push(module.idx());
            }
          }
        }
      }
      for entry in entry_points
        .iter()
        .chain(rest.iter().filter(|entry| matches!(entry.kind, EntryPointKind::EmittedUserDefined)))
      {
        graph.add_edge(root, NodeIndex::new(entry.idx.index()), ());
      }
      let dominators = simple_fast(&graph, root);
      let input_entries: FxHashSet<_> = entry_points.iter().map(|entry| entry.idx).collect();
      let static_closure = |start: ModuleIdx| {
        let mut visited = FxHashSet::default();
        let mut stack = vec![start];
        while let Some(importer) = stack.pop() {
          if visited.insert(importer) {
            for record in modules[importer].import_records() {
              if record.kind.is_static()
                && let Some(dependency) = record.resolved_module
              {
                stack.push(dependency);
              }
            }
          }
        }
        visited
      };
      let mut evaluated = FxHashSet::default();
      for entry in entry_points.iter() {
        evaluated.extend(static_closure(entry.idx));
      }
      let mut placed = vec![false; entry_order.len()];
      let mut order = Vec::with_capacity(entry_order.len());
      for (index, &idx) in entry_order.iter().enumerate() {
        if placed[index] {
          continue;
        }
        let mut loaders = Vec::new();
        let mut ancestor = dominators.immediate_dominator(NodeIndex::new(idx.index()));
        while let Some(node) = ancestor {
          if node == root {
            break;
          }
          let dominator = ModuleIdx::new(node.index());
          if input_entries.contains(&dominator) {
            break;
          }
          if let Some(before) = entry_order.get_index_of(&dominator) {
            loaders.push(before);
          }
          ancestor = dominators.immediate_dominator(node);
        }
        let mut imported = vec![false; loaders.len()];
        let mut remaining = loaders.len();
        let mut visited = FxHashSet::default();
        let mut stack = vec![idx];
        while remaining > 0
          && let Some(importer) = stack.pop()
        {
          if visited.insert(importer) {
            if let Some(position) =
              loaders.iter().position(|&before| entry_order[before] == importer)
            {
              imported[position] = true;
              remaining -= 1;
            }
            for record in modules[importer].import_records() {
              if record.kind.is_static()
                && let Some(dependency) = record.resolved_module
              {
                stack.push(dependency);
              }
            }
          }
        }
        for (position, &before) in loaders.iter().enumerate().rev() {
          let loader = entry_order[before];
          if !imported[position] || evaluated.contains(&loader) {
            continue;
          }
          let mut importers = FxHashSet::default();
          let mut stack = static_closure(idx)
            .into_iter()
            .filter(|module| !evaluated.contains(module))
            .collect::<Vec<_>>();
          while let Some(importee) = stack.pop() {
            if importers.insert(importee) {
              stack.extend(static_importers[importee].iter().copied());
            }
          }
          let exclusively_dominated = entry_points
            .iter()
            .chain(rest.iter())
            .filter(|entry| importers.contains(&entry.idx))
            .all(|entry| {
              dominators
                .dominators(NodeIndex::new(entry.idx.index()))
                .is_some_and(|mut chain| chain.any(|node| node.index() == loader.index()))
            });
          if !exclusively_dominated {
            continue;
          }
          let loader_closure = static_closure(loader);
          let has_tla_or_external = loader_closure.iter().any(|&idx| {
            let module = &modules[idx];
            module
              .as_normal()
              .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait))
              || module.import_records().iter().any(|record| {
                record.kind.is_static()
                  && record
                    .resolved_module
                    .and_then(|dependency| modules[dependency].as_external())
                    .is_some_and(|external| !is_nodejs_builtin_module(external.id.as_str()))
              })
          });
          if has_tla_or_external {
            continue;
          }
          placed[before] = true;
          order.push(before);
          evaluated.extend(loader_closure);
        }
        placed[index] = true;
        order.push(index);
        evaluated.extend(static_closure(idx));
      }
      let mut position = vec![0; entry_order.len()];
      for (at, &index) in order.iter().enumerate() {
        position[index] = at;
      }
      rest.sort_by_cached_key(|entry| position[entry_order.get_index_of(&entry.idx).unwrap()]);
    }

    entry_points.extend(rest);
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    self.sort_modules();
    self.determine_module_exports_kind();
    self.determine_safely_merge_cjs_ns();
    self.wrap_modules();
    self.generate_lazy_export();
    self.determine_side_effects();
    self.bind_imports_and_exports();
    self.create_exports_for_ecma_modules();
    self.promote_deferred_write_constants();
    self.exclude_cyclic_vars_from_inline_const();
    let unreachable_import_expression_node_ids = self.cross_module_optimization();
    self.exclude_jsx_element_names_from_inline_const();
    self.refine_stmt_side_effects_with_imported_constants();
    // The passes above only relax statement flags. Module verdicts must follow before
    // `reference_needed_symbols` derives import flags from them.
    self.recompute_analyzed_side_effects();
    self.reference_needed_symbols();
    self.prune_constant_branches();
    self.include_statements(&unreachable_import_expression_node_ids);
    self.compute_tla();
    self.assert_promoted_writers_excluded();
    self.check_lazy_barrel_uncertain_bindings();
    self.patch_module_dependencies();

    tracing::trace!(
      modules = self.metas.len(),
      included_modules = self.metas.iter().filter(|meta| meta.is_included).count(),
      resolved_exports = self.metas.iter().map(|meta| meta.resolved_exports.len()).sum::<usize>(),
      "link metadata ready"
    );

    (
      LinkStageOutput {
        module_table: self.module_table,
        entries: self.entries,
        sorted_modules: self.sorted_modules,
        metas: self.metas,
        symbol_db: self.symbols,
        stmt_infos: self.stmt_infos,
        runtime: self.runtime,
        diagnostics: self.diagnostics,
        used_external_symbols: self.used_external_symbols,
        retained_export_symbols: RetainedExportSymbols::default(),
        dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map,
        safely_merge_cjs_ns_map: self.safely_merge_cjs_ns_map,
        external_import_namespace_merger: self.external_import_namespace_merger,
        overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map,
        entry_point_to_reference_ids: self.entry_point_to_reference_ids,
        global_constant_symbol_map: self.global_constant_symbol_map,
        normal_symbol_exports_chain_map: self.normal_symbol_exports_chain_map,
        star_reexport_records_by_imported_symbol: self.star_reexport_records_by_imported_symbol,
        member_read_star_reexport_paths: self.member_read_star_reexport_paths,
        kept_inlined_constant_reads: self.kept_inlined_constant_reads,
        user_defined_entry_modules: self.user_defined_entry_modules,
        has_enum_inlining: self.has_enum_inlining,
      },
      self.ast_table,
      self.used_symbol_refs_builder,
    )
  }

  /// A helper function used to debug symbol in link process
  /// given any `SymbolRef` the function will return the string representation of the symbol
  /// format: `${stable_id} -> ${symbol_name}`
  #[cfg(debug_assertions)]
  #[cfg_attr(debug_assertions, expect(unused))]
  pub fn debug_symbol_ref(&self, symbol_ref: SymbolRef) -> String {
    common_debug_symbol_ref(symbol_ref, &self.module_table.modules, &self.symbols)
  }
}
