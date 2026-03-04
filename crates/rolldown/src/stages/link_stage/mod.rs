use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  ConstExportMeta, EntryPoint, EntryPointKind, FlatOptions, ImportKind, LinkKernel, ModuleIdx,
  ModuleTable, PreserveEntrySignatures, RuntimeModuleBrief, SymbolRef, SymbolRefDb,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::BuildDiagnostic;
#[cfg(target_family = "wasm")]
use rolldown_utils::rayon::IteratorExt as _;
use rolldown_utils::{
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  type_alias::IndexEcmaAst,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

use super::scan_stage::NormalizedScanStageOutput;

mod bind_imports_and_exports;
mod compute_tla;
mod create_exports_for_ecma_modules;
mod cross_module_optimization;
mod determine_module_exports_kind;
mod generate_lazy_export;
mod patch_module_dependencies;
mod reference_needed_symbols;
mod sort_modules;
mod tree_shaking;

pub use tree_shaking::{
  ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  include_statements::{
    IncludeContext, SymbolIncludeReason, include_runtime_symbol, include_symbol,
  },
};
mod oxc_conversions;
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
  pub ast_table: IndexEcmaAst,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub symbol_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  /// https://rollupjs.org/plugin-development/#this-emitfile
  /// Used to store `preserveSignature` specified with `this.emitFile` in plugins.
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
}

impl LinkStageOutput {
  /// Resolve a symbol to its canonical form.
  ///
  /// After `flatten_all_chains()` runs at the start of the generate stage,
  /// Rolldown's SymbolRefDb contains all links (graph links mirrored from
  /// `match_imports_collect` + facade links for wrapper/shim symbols) with
  /// flattened chains. A single lookup is sufficient and avoids the overhead
  /// of dual-resolution through both the graph and Rolldown DBs.
  pub fn graph_canonical_ref(&self, sym: SymbolRef) -> SymbolRef {
    self.symbol_db.canonical_ref_for(sym)
  }
}

#[derive(Debug)]
pub struct LinkStage<'a> {
  pub module_table: ModuleTable,
  pub entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>>,
  pub symbols: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub sorted_modules: Vec<ModuleIdx>,
  pub metas: LinkingMetadataVec,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub ast_table: IndexEcmaAst,
  pub options: &'a SharedOptions,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
  pub safely_merge_cjs_ns_map: FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  pub flat_options: FlatOptions,
  pub side_effects_free_function_symbol_ref: FxHashSet<SymbolRef>,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  pub tla_module_count: usize,
  pub link_kernel: LinkKernel,
}

impl<'a> LinkStage<'a> {
  /// Resolve a symbol to its canonical form.
  ///
  /// After `bind_imports_and_exports` mirrors all graph links to Rolldown's
  /// SymbolRefDb, a single walk through Rolldown's DB is sufficient.
  pub fn graph_canonical_ref(&self, sym: SymbolRef) -> SymbolRef {
    self.symbols.canonical_ref_for(sym)
  }

  pub fn new(mut scan_stage_output: NormalizedScanStageOutput, options: &'a SharedOptions) -> Self {
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

    // We need to preserve the original order of user defined entry points.
    let mut rest = scan_stage_output
      .entry_points
      .extract_if(0.., |item| !matches!(item.kind, EntryPointKind::UserDefined))
      .collect_vec();

    rest.sort_by_cached_key(|item| {
      (item.kind, scan_stage_output.module_table.modules[item.idx].id().as_str())
    });

    scan_stage_output.entry_points.extend(rest);

    let entries = {
      let mut entries: FxIndexMap<ModuleIdx, Vec<EntryPoint>> = FxIndexMap::default();
      for entry in scan_stage_output.entry_points {
        entries.entry(entry.idx).or_default().push(entry);
      }
      entries
    };

    let mut result = Self {
      sorted_modules: Vec::new(),
      global_constant_symbol_map: constant_symbol_map,
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
              // when `inlineDynamicImports` is enabled.
              ImportKind::DynamicImport | ImportKind::Require => None,
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
      entries,
      symbols: scan_stage_output.symbol_ref_db,
      runtime: scan_stage_output.runtime,
      warnings: scan_stage_output.warnings,
      errors: vec![],
      ast_table: scan_stage_output.index_ecma_ast,
      dynamic_import_exports_usage_map: scan_stage_output.dynamic_import_exports_usage_map,
      options,
      used_symbol_refs: FxHashSet::default(),
      safely_merge_cjs_ns_map: FxHashMap::default(),
      normal_symbol_exports_chain_map: FxHashMap::default(),
      external_import_namespace_merger: FxHashMap::default(),
      overrode_preserve_entry_signature_map: scan_stage_output
        .overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: scan_stage_output.entry_point_to_reference_ids,
      flat_options: scan_stage_output.flat_options,
      side_effects_free_function_symbol_ref: FxHashSet::default(),
      user_defined_entry_modules: scan_stage_output.user_defined_entry_modules,
      tla_module_count: scan_stage_output.tla_module_count,
      link_kernel: scan_stage_output.link_kernel,
    };

    // Update graph entries to match the reordered entry points.
    // The scan stage populated the graph with original entry order, but
    // LinkStage::new() reorders entries (user-defined first, then sorted).
    // The graph's entry order affects DFS traversal in compute_exec_order.
    let graph_entries: Vec<oxc_module_graph::types::ModuleIdx> = result
      .entries
      .keys()
      .map(|&idx| oxc_module_graph::types::ModuleIdx::from_usize(idx.index()))
      .collect();
    result.link_kernel.graph.set_entries(graph_entries);

    result
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> LinkStageOutput {
    self.sort_modules();
    self.compute_tla();
    self.determine_module_exports_kind();
    self.determine_safely_merge_cjs_ns();
    // No sync needed — determine_module_exports_kind writes to graph directly.
    self.wrap_modules();
    // No sync needed — wrap_modules writes has_dynamic_exports + wrap_kind to graph directly.
    self.generate_lazy_export();
    // No sync needed — generate_lazy_export syncs its own changes to graph
    // for only the affected lazy-export modules (targeted, not full-table).
    self.determine_side_effects();
    self.bind_imports_and_exports();

    // Free graph memory — all graph-dependent algorithms are done.
    // Subsequent passes only read Rolldown's own SymbolRefDb and metas.
    // Reclaims ~48KB/module of duplicated data before the heaviest phases.
    self.link_kernel.graph = oxc_module_graph::ModuleGraph::default();

    self.create_exports_for_ecma_modules();
    self.reference_needed_symbols();
    let unreachable_import_expression_addrs = self.cross_module_optimization();
    self.include_statements(&unreachable_import_expression_addrs);
    self.patch_module_dependencies();

    tracing::trace!("meta {:#?}", self.metas.iter_enumerated().collect::<Vec<_>>());

    LinkStageOutput {
      module_table: self.module_table,
      entries: self.entries,
      sorted_modules: self.sorted_modules,
      metas: self.metas,
      symbol_db: self.symbols,
      runtime: self.runtime,
      warnings: self.warnings,
      errors: self.errors,
      ast_table: self.ast_table,
      used_symbol_refs: self.used_symbol_refs,
      dynamic_import_exports_usage_map: self.dynamic_import_exports_usage_map,
      safely_merge_cjs_ns_map: self.safely_merge_cjs_ns_map,
      external_import_namespace_merger: self.external_import_namespace_merger,
      overrode_preserve_entry_signature_map: self.overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: self.entry_point_to_reference_ids,
      global_constant_symbol_map: self.global_constant_symbol_map,
      normal_symbol_exports_chain_map: self.normal_symbol_exports_chain_map,
      user_defined_entry_modules: self.user_defined_entry_modules,
    }
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
