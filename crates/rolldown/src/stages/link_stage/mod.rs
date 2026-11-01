use arcstr::ArcStr;
use itertools::Itertools;
use oxc::span::Span;
use oxc_index::IndexVec;
#[cfg(debug_assertions)]
use rolldown_common::common_debug_symbol_ref;
use rolldown_common::{
  ConstExportMeta, DependedRuntimeHelperMap, EntryPoint, EntryPointKind, FlatOptions, ImportKind,
  IndexModules, ModuleId, ModuleIdx, ModuleTable, NormalModule, PreserveEntrySignatures,
  RetainedExportSymbols, RuntimeModuleBrief, SymbolRef, SymbolRefDb, UsedExternalSymbols,
  UsedSymbolRefsBuilder, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_error::Diagnostics;
#[cfg(target_family = "wasm")]
use rolldown_utils::rayon::IteratorExt as _;
use rolldown_utils::{
  IndexBitSet,
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
pub(crate) use bind_imports_and_exports::{
  appended_module_exports, bind_appended_module_imports, commonjs_exports_public,
  inlined_commonjs_constants, split_json_defaults,
};
mod compute_tla;
mod create_exports_for_ecma_modules;
mod cross_module_optimization;
mod deferred_imports;
mod determine_module_exports_kind;
pub(crate) use determine_module_exports_kind::commonjs_interop;
mod generate_lazy_export;
mod patch_module_dependencies;
mod reference_needed_symbols;
mod refine_stmt_side_effects;
mod sort_modules;
mod temporal_bindings;
mod tree_shaking;

pub(crate) use tree_shaking::determine_side_effects::{
  linked_side_effects, propagate_side_effects,
};
pub use tree_shaking::{
  IncludeContext, ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec,
  SymbolIncludeReason, compute_body_demand_keys, include_runtime_symbol, include_symbol,
};
mod wrapping;
pub(crate) use wrapping::{cjs_wrapper_name, cjs_wrapper_stmt_info, dynamic_exports};

/// Information about safely merged CJS namespaces for a module
#[derive(Debug, Default, Clone)]
pub struct SafelyMergeCjsNsInfo {
  /// Namespace symbol refs that can be merged into a single binding
  pub namespace_refs: Vec<SymbolRef>,
  /// Whether this CJS module needs `__toESM` interop (has namespace or default imports)
  pub needs_interop: bool,
}

/// The modules that a module table keeps though the graph's entries no longer reach them. A
/// module HMR session keeps the modules that an update leaves unreachable until it builds a fresh
/// graph, which wouldn't load them, so linking passes over them and their records import nothing
/// (#660).
#[derive(Debug, Default, Clone)]
pub struct UnreachableModules {
  idxs: FxHashSet<ModuleIdx>,
  ids: FxHashSet<ModuleId>,
  discovery_order: IndexVec<ModuleIdx, u32>,
}

impl UnreachableModules {
  /// Whether the graph's entries reach `idx`.
  pub fn reaches(&self, idx: ModuleIdx) -> bool {
    !self.idxs.contains(&idx)
  }

  /// The 0-based order in which a fresh breadth-first scan from the roots discovers `idx`, or
  /// `u32::MAX` if unreachable (#681).
  pub fn discovery_order(&self, idx: ModuleIdx) -> u32 {
    self.discovery_order.get(idx).copied().unwrap_or(u32::MAX)
  }

  /// Whether a module that the graph reaches imports `module` with `import()` or names it in
  /// `import.meta.hot.accept()`, as the importers in its `dynamic_importers` do.
  pub fn dynamically_imported(&self, module: &NormalModule) -> bool {
    module.ecma_view.dynamic_importers.iter().any(|importer| !self.ids.contains(importer))
  }

  /// Whether a static `import` or dynamic `import()` of a module that the graph reaches imports
  /// `idx` (#660, #821). An `import.meta.hot.accept()` is not an import and does not evaluate a
  /// CommonJS record eagerly.
  pub fn imported_by_static_or_dynamic_import(
    &self,
    modules: &IndexModules,
    idx: ModuleIdx,
  ) -> bool {
    modules[idx].as_normal().is_some_and(|module| {
      module
        .ecma_view
        .importers_idx
        .iter()
        .chain(&module.ecma_view.dynamic_importers_idx)
        .any(|&importer| {
          self.reaches(importer)
            && modules[importer].as_normal().is_some_and(|importer| {
              importer.import_records.iter().any(|record| {
                matches!(record.kind, ImportKind::Import | ImportKind::DynamicImport)
                  && record.resolved_module == Some(idx)
              })
            })
        })
    })
  }
}

/// What a scan from a graph's user-defined entries and its runtime module loads: the modules that
/// resolved import records of any kind reach from them, in discovery order (#681), and the dynamic
/// entries that those modules import with `import()`.
pub(crate) struct Reachability {
  reached: IndexBitSet<ModuleIdx>,
  dynamically_imported: IndexBitSet<ModuleIdx>,
  discovery_order: IndexVec<ModuleIdx, u32>,
}

impl Reachability {
  pub(crate) fn new(modules: &IndexModules, roots: impl IntoIterator<Item = ModuleIdx>) -> Self {
    let mut reached = IndexBitSet::new(modules.len());
    let mut dynamically_imported = IndexBitSet::new(modules.len());
    let mut discovery_order = IndexVec::from_vec(vec![u32::MAX; modules.len()]);
    let mut next_order = 0u32;
    let mut pending = Vec::new();
    for idx in roots {
      if reached.set_bit(idx) {
        discovery_order[idx] = next_order;
        next_order += 1;
        pending.push(idx);
      }
    }
    let mut head = 0;
    while let Some(&idx) = pending.get(head) {
      head += 1;
      for record in modules[idx].import_records() {
        let Some(target) = record.resolved_module else {
          continue;
        };
        if record.kind == ImportKind::DynamicImport {
          dynamically_imported.set_bit(target);
        }
        if reached.set_bit(target) {
          discovery_order[target] = next_order;
          next_order += 1;
          pending.push(target);
        }
      }
    }
    Self { reached, dynamically_imported, discovery_order }
  }

  pub(crate) fn reaches(&self, idx: ModuleIdx) -> bool {
    self.reached.has_bit(idx)
  }

  /// The 0-based order in which a fresh breadth-first scan from the roots discovers `idx`, or
  /// `u32::MAX` if unreachable (#681).
  pub(crate) fn discovery_order(&self, idx: ModuleIdx) -> u32 {
    self.discovery_order.get(idx).copied().unwrap_or(u32::MAX)
  }

  /// Whether the scan makes `entry` an entry: a user-defined one, or a dynamic one that a module
  /// that it reaches imports with `import()`.
  pub(crate) fn keeps_entry(&self, entry: &EntryPoint) -> bool {
    entry.kind.is_user_defined() || self.dynamically_imported.has_bit(entry.idx)
  }

  pub(crate) fn unreachable(&self, modules: &IndexModules) -> UnreachableModules {
    let mut unreachable = UnreachableModules {
      idxs: FxHashSet::default(),
      ids: FxHashSet::default(),
      discovery_order: self.discovery_order.clone(),
    };
    for (idx, module) in modules.iter_enumerated() {
      if !self.reached.has_bit(idx) {
        unreachable.idxs.insert(idx);
        unreachable.ids.insert(module.id().clone());
      }
    }
    unreachable
  }
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
  /// The JSON modules whose default object a module that the graph reaches writes or lets escape,
  /// which linking therefore keeps whole for the modules that read its properties, rather than
  /// split into their bindings (`resolve_member_expr_refs`).
  pub non_splittable_json_defaults: FxHashSet<ModuleIdx>,
  pub normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub star_reexport_records_by_imported_symbol:
    FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, rolldown_common::ImportRecordIdx)>>>,
  pub member_read_star_reexport_paths: Vec<MemberReadStarReexportPath>,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// The modules that linking passed over, as the graph's entries no longer reach them.
  pub unreachable_modules: UnreachableModules,
  /// True if any module has enum member values to inline. Computed once to avoid
  /// repeated full module table scans.
  pub has_enum_inlining: bool,
  pub pre_lazy_export_asts: FxHashMap<ModuleIdx, rolldown_ecmascript::EcmaAst>,
  pub pre_lazy_export_symbols: FxHashMap<ModuleIdx, rolldown_common::SymbolRefDbForModule>,
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
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub overrode_preserve_entry_signature_map: FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  pub entry_point_to_reference_ids: FxHashMap<EntryPoint, Vec<ArcStr>>,
  pub global_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  /// See [`LinkStageOutput::non_splittable_json_defaults`].
  pub non_splittable_json_defaults: FxHashSet<ModuleIdx>,
  pub flat_options: FlatOptions,
  pub user_defined_entry_modules: FxHashSet<ModuleIdx>,
  /// Found before the passes that read the module table, which pass over these modules.
  pub unreachable_modules: UnreachableModules,
  pub tla_module_count: usize,
  /// Centralized map of modules that use top-level `await` → span of the
  /// first TLA keyword. Populated during the scan stage.
  pub tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
  /// Computed during `include_statements`, reused when building `LinkStageOutput`.
  pub has_enum_inlining: bool,
  pub pre_lazy_export_asts: FxHashMap<ModuleIdx, rolldown_ecmascript::EcmaAst>,
  pub pre_lazy_export_symbols: FxHashMap<ModuleIdx, rolldown_common::SymbolRefDbForModule>,
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
    // since constant export is spared in most of time, aggregate them would make searching more efficient
    let constant_symbol_map = if options.optimization.is_inline_const_enabled() {
      scan_stage_output
        .module_table
        .modules
        .par_iter_mut()
        .filter_map(|m| {
          let m = m.as_normal_mut()?;
          Some(
            std::mem::take(&mut m.constant_export_map)
              .into_iter()
              .map(|(symbol_id, v)| {
                let symbol_ref = SymbolRef {
                  owner: m.idx,
                  symbol: symbol_id,
                };
                (symbol_ref, v)
              }),
          )
        })
        .flatten_iter()
        .collect::<FxHashMap<SymbolRef, ConstExportMeta>>()
    } else {
      FxHashMap::default()
    };

    // We need to preserve the original order of user defined entry points.
    let mut rest = scan_stage_output
      .entry_points
      .extract_if(0.., |item| {
        !matches!(item.kind, EntryPointKind::UserDefined)
      })
      .collect_vec();

    rest.sort_by_cached_key(|item| {
      (
        item.kind,
        scan_stage_output.module_table.modules[item.idx]
          .id()
          .as_str(),
      )
    });

    scan_stage_output.entry_points.extend(rest);

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
      non_splittable_json_defaults: FxHashSet::default(),
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
      metas: {
        use rolldown_utils::rayon::IntoParallelRefIterator as _;
        scan_stage_output
          .module_table
          .modules
          .par_iter()
          .map(|module| {
            let mut meta = LinkingMetadata::default();
            meta.dependencies = module
              .import_records()
              .iter()
              .filter_map(|rec| match rec.kind {
                ImportKind::DynamicImport | ImportKind::Require | ImportKind::HotAccept => None,
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
          .collect::<Vec<_>>()
          .into()
      },
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
      external_import_namespace_merger: FxHashMap::default(),
      overrode_preserve_entry_signature_map: scan_stage_output
        .overrode_preserve_entry_signature_map,
      entry_point_to_reference_ids: scan_stage_output.entry_point_to_reference_ids,
      flat_options: scan_stage_output.flat_options,
      user_defined_entry_modules: scan_stage_output.user_defined_entry_modules,
      unreachable_modules: UnreachableModules::default(),
      tla_module_count: scan_stage_output.tla_module_count,
      tla_keyword_span_map: scan_stage_output.tla_keyword_span_map,
      has_enum_inlining: false,
      pre_lazy_export_asts: FxHashMap::default(),
      pre_lazy_export_symbols: FxHashMap::default(),
    }
  }

  /// An `import()` resolves its promise with the module's namespace, which reads the namespace's
  /// `then` and calls it with the namespace as `this`. A module that exports `then` therefore keeps
  /// its whole namespace for its dynamic importers, whatever names they read (#664), unless its
  /// `then` is a module namespace (`exports_namespace_as_then`).
  fn keep_thenable_namespaces(&mut self) {
    for (idx, usage) in &mut self.dynamic_import_exports_usage_map {
      if !matches!(usage, DynamicImportExportsUsage::Complete)
        && self.metas[*idx].sorted_and_non_ambiguous_resolved_exports.contains_key("then")
        && !self.module_table[*idx].as_normal().is_some_and(exports_namespace_as_then)
      {
        *usage = DynamicImportExportsUsage::Complete;
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn link(mut self) -> (LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder) {
    self.exclude_unreachable_modules();
    self.collect_deferred_imports();
    self.sort_modules();
    self.compute_tla();
    self.determine_module_exports_kind();
    self.determine_safely_merge_cjs_ns();
    self.wrap_modules();
    self.generate_lazy_export();
    self.determine_side_effects();
    self.bind_imports_and_exports();
    self.keep_thenable_namespaces();
    self.create_exports_for_ecma_modules();
    self.preserve_temporal_bindings();
    self.reference_async_evaluation_helpers();
    let unreachable_import_expression_node_ids = self.cross_module_optimization();
    self.refine_stmt_side_effects_with_imported_constants();
    // The passes above only relax statement flags. Module verdicts must follow before
    // `reference_needed_symbols` derives import flags from them.
    self.recompute_analyzed_side_effects();
    self.reference_needed_symbols();
    self.include_statements(&unreachable_import_expression_node_ids);
    self.patch_module_dependencies();

    tracing::trace!(
      modules = self.metas.len(),
      included_modules = self.metas.iter().filter(|meta| meta.is_included).count(),
      resolved_exports = self
        .metas
        .iter()
        .map(|meta| meta.resolved_exports.len())
        .sum::<usize>(),
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
        non_splittable_json_defaults: self.non_splittable_json_defaults,
        normal_symbol_exports_chain_map: self.normal_symbol_exports_chain_map,
        star_reexport_records_by_imported_symbol: self.star_reexport_records_by_imported_symbol,
        member_read_star_reexport_paths: self.member_read_star_reexport_paths,
        user_defined_entry_modules: self.user_defined_entry_modules,
        unreachable_modules: self.unreachable_modules,
        has_enum_inlining: self.has_enum_inlining,
        pre_lazy_export_asts: self.pre_lazy_export_asts,
        pre_lazy_export_symbols: self.pre_lazy_export_symbols,
      },
      self.ast_table,
      self.used_symbol_refs_builder,
    )
  }

  /// Finds the modules that the graph's entries no longer reach, which the passes that read the
  /// module table pass over, and drops the dynamic entries that no module that they reach imports
  /// any more, with the statements of unreachable importers that created them. Sorting would
  /// otherwise order such a dead entry, and what it imports, before inclusion removed it.
  fn exclude_unreachable_modules(&mut self) {
    let roots = self
      .entries
      .values()
      .flatten()
      .filter(|entry| entry.kind.is_user_defined())
      .map(|entry| entry.idx)
      .chain(self.user_defined_entry_modules.iter().copied())
      .chain(std::iter::once(self.runtime.id()));
    let reachability = Reachability::new(&self.module_table.modules, roots);
    self.entries.retain(|_, entries| {
      entries.retain_mut(|entry| {
        entry.related_stmt_infos.retain(|(importer, ..)| reachability.reaches(*importer));
        reachability.keeps_entry(entry)
      });
      !entries.is_empty()
    });
    self.unreachable_modules = reachability.unreachable(&self.module_table.modules);
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

/// Whether `module`'s own `then` export is a module namespace (`export * as then from`, or an
/// `import * as` that it exports as `then`). A namespace object isn't callable, so resolving an
/// `import()` of the module doesn't call it, and the import keeps only the names that it reads.
pub(crate) fn exports_namespace_as_then(module: &NormalModule) -> bool {
  module.named_exports.get("then").is_some_and(|export| {
    module.named_imports.get(&export.referenced).is_some_and(|import| import.imported.is_star())
  })
}
