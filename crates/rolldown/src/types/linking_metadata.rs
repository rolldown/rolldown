use crate::stages::link_stage::{ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec};
use oxc::semantic::NodeId;
use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  ConcatenateWrappedModuleKind, EntryPointKind, ImportRecordIdx, ImportRecordMeta,
  MemberExprRefResolutionMap, ModuleIdx, ModuleNamespaceIncludedReason, ModuleType, NormalModule,
  OutputFormat, ResolvedExport, RuntimeHelper, StmtInfoIdx, SymbolRef, SymbolRefDb, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::IndexBitSet;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

/// The module that a deferred access (`ns.foo`) evaluates first.
#[derive(Debug, Clone, Copy)]
pub enum DeferredAccess {
  /// An ESM module: `(init_x(), <access>)`.
  Esm(ModuleIdx),
  /// The import record of a CommonJS module:
  /// `(import_x || (import_x = __toESM(require_x())), <access>)`.
  Cjs(ImportRecordIdx),
}

/// The deferred namespace object of one `import defer * as ns from './x'`, where `./x` is a
/// CommonJS module: `x_deferred = __deferNs(() => import_x || (import_x = __toESM(require_x())))`.
/// The importer declares it next to `import_x`, and the two share one namespace object.
#[derive(Debug, Clone, Copy)]
pub struct DeferredCjsNamespace {
  pub symbol_ref: SymbolRef,
  /// The facade statement that declares `symbol_ref`.
  pub stmt_info_idx: StmtInfoIdx,
}

/// Module metadata about linking
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools)]
pub struct LinkingMetadata {
  /// A module could be wrapped for some reasons, eg. cjs module need to be wrapped with commonjs runtime function.
  /// The `wrap_ref` is the binding identifier that store return value of executed the wrapper function.
  ///
  /// ## Example
  ///
  /// ```js
  /// // cjs.js
  /// module.exports = {}
  /// ```
  ///
  /// will be transformed to
  ///
  /// ```js
  /// // cjs.js
  /// var require_cjs = __commonJS({
  ///   'cjs.js'(exports, module) {
  ///     module.exports = {}
  ///
  ///   }
  /// });
  /// ```
  ///
  /// `wrapper_ref` is the `require_cjs` identifier in above example.
  pub wrapper_ref: Option<SymbolRef>,
  pub wrapper_stmt_info: Option<StmtInfoIdx>,
  /// The module representation decided during linking.
  wrap_kind: WrapKind,
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<CompactStr, ResolvedExport>,
  /// Store the names of exclude ambiguous resolved exports.
  /// It will be used to generate chunk exports and module namespace binding.
  /// The second element means if the export is came from commonjs module.
  pub sorted_and_non_ambiguous_resolved_exports: FxIndexMap<CompactStr, bool>,
  /// Direct `export *` record that first contributed each flattened resolved export.
  pub star_export_record_by_name: FxHashMap<CompactStr, ImportRecordIdx>,
  // If a esm module has export star from commonjs, it will be marked as ESMWithDynamicFallback at linker.
  // The unknown export name will be resolved at runtime.
  // esbuild add it to `ExportKind`, but the linker shouldn't mutate the module.
  pub has_dynamic_exports: bool,
  pub shimmed_missing_exports: FxHashMap<CompactStr, SymbolRef>,
  pub required_by_other_module: bool,

  // Entry chunks need to generate code that doesn't belong to any module. This is the list of symbols are referenced by the
  // generated code. Tree-shaking will cares about these symbols to make sure they are not removed.
  // The second element means if the symbol is a facade symbol.
  pub referenced_symbols_by_entry_point_chunk: Vec<(SymbolRef, bool)>,

  /// The dependencies of the module. It means if you want include this module, you need to include these dependencies too.
  pub dependencies: FxIndexSet<ModuleIdx>,
  /// The subset of module graph edges that force the target module to be loaded when this module
  /// executes, used by code splitting to compute per-entry reachability
  /// (`determine_reachable_modules_for_entry`):
  ///
  /// - modules owning the canonical symbols referenced by this module's included statements
  ///   (these are what become cross-chunk symbol imports), and
  /// - import-record targets whose evaluation has side effects, mirroring both
  ///   `include_side_effectful_dependencies` in tree-shaking and the bare-import emission in
  ///   `compute_cross_chunk_links`.
  ///
  /// Unlike [`Self::dependencies`], a side-effect-free module imported only for bindings that
  /// canonically resolve elsewhere (e.g. a pure barrel re-exporting them) is *not* a load
  /// dependency, so an entry that never uses the barrel's own code doesn't pull the barrel (or
  /// its subtree) into its chunk group (#8920). Populated by `patch_module_dependencies`; with
  /// tree-shaking disabled it equals [`Self::dependencies`].
  pub load_dependencies: FxIndexSet<ModuleIdx>,
  /// Retained evaluation dependencies, excluding entry targets kept only for placement.
  pub execution_dependencies: FxIndexSet<ModuleIdx>,
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: MemberExprRefResolutionMap,
  /// Results of the `'key' in ns` checks (see `EcmaView::namespace_in_checks`) that are known at
  /// link time.
  pub folded_namespace_in_checks: FxHashMap<NodeId, bool>,
  /// Set on each target of `import defer` and on each module of its static dependency closure.
  /// The ESM wrapper of such a module is a hoisted function, so that a call from a cyclic
  /// importer that runs before the declaration works.
  pub wrapped_by_import_defer: bool,
  /// Set on each ESM module that an `import.defer()` call targets. The chunk that holds the
  /// module must not evaluate it when the chunk loads, and each `import()` or `import.defer()` of
  /// the module reads `init_x`, `x_exports` and `x_deferred` from that chunk.
  pub dynamic_defer_target: bool,
  /// Set on each external module that a module of a deferred closure loads lazily in CommonJS
  /// output, with an included import that needs `__toESMCached` (`lazy_external_import_stmt`).
  /// Each chunk then renders its own binding of the external with `__toESMCached` too, so that
  /// all these loads give one namespace object (`render_cjs_chunk_imports`).
  pub has_lazy_namespace_load: bool,
  /// The deferred namespace object of this module (`x_deferred`), when an `import defer * as ns`
  /// or an `import.defer()` targets this ESM module. Each deferred star import of the module
  /// links to this symbol, so that all of them read one object, also through `export { ns }`.
  pub deferred_namespace_ref: Option<SymbolRef>,
  /// The facade statement that declares `deferred_namespace_ref`.
  pub deferred_namespace_stmt_info: Option<StmtInfoIdx>,
  /// Set when a deferred access must observe this module: the module is in a strongly connected
  /// component that contains a target of `import defer`, in the graph of the requested modules of
  /// all phases. The wrapper is then `__esmState`, which keeps a record of the evaluation state,
  /// and a deferred access checks the records first (`__esmDefer`). The list has the tracked
  /// modules that this module requests, in source order, for the ready check.
  pub import_defer_tracked_deps: Option<Vec<ModuleIdx>>,
  /// The local bindings of `import defer * as ns from './x'` in this module where `./x` is a
  /// CommonJS module, mapped to the import record of `./x`. An access through the local binding
  /// reads `import_x` directly (`DeferredAccess::Cjs`).
  pub deferred_cjs_namespace_imports: FxHashMap<SymbolRef, ImportRecordIdx>,
  /// The deferred namespace object of each import record in `deferred_cjs_namespace_imports`.
  /// The local binding links to it, so that an escape (`f(ns)`, `export { ns }`) reads the object.
  pub deferred_cjs_namespaces: FxHashMap<ImportRecordIdx, DeferredCjsNamespace>,
  /// The accesses to a deferred namespace (`ns.foo`, `'foo' in ns`) that evaluate the deferred
  /// module first. The keys are the node ids of `resolved_member_expr_refs` and
  /// `folded_namespace_in_checks`.
  pub deferred_access_targets: FxHashMap<NodeId, DeferredAccess>,
  pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
  pub is_tla_or_contains_tla_dependency: bool,
  pub concatenated_wrapped_module_kind: ConcatenateWrappedModuleKind,
  /// Used to to track a facade binding referenced cjs module
  /// included reexport symbol from commonjs module
  pub named_import_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx>,
  pub import_record_ns_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx>,
  /// Currently our symbol link system could only link one symbol to another one, but for commonjs
  /// tree shaking, when one symbol was linked it may not only link the namespace ref symbol, and
  /// also need to link the exported facade symbol.
  pub included_commonjs_export_symbol: FxHashSet<SymbolRef>,
  pub depended_runtime_helper: RuntimeHelper,
  /// Whether this module needs the runtime chunk loaded for its side effects.
  /// Set when the runtime module has side effects (e.g. dev/HMR mode).
  pub has_side_effectful_runtime_dep: bool,
  pub module_namespace_included_reason: ModuleNamespaceIncludedReason,
  /// Final decision on whether this module's namespace object is retained in the output.
  /// Computed by [`crate::stages::generate_stage`]'s `finalized_module_namespace_ref_usage`
  /// from `module_namespace_included_reason` together with the module's `exports_kind` and
  /// `has_dynamic_exports`; only meaningful for passes that run after it.
  pub namespace_included: bool,
  /// Tracks which statements in this module are included after tree-shaking.
  /// Each entry corresponds to a statement in the module's `stmt_infos`.
  pub stmt_info_included: IndexBitSet<StmtInfoIdx>,
  /// Tracks whether the module is included after tree-shaking.
  pub is_included: bool,
}

impl LinkingMetadata {
  pub fn canonical_exports(
    &self,
    needs_commonjs_export: bool,
  ) -> impl Iterator<Item = (&CompactStr, &ResolvedExport)> {
    self.sorted_and_non_ambiguous_resolved_exports.iter().filter_map(
      move |(name, came_from_cjs)| {
        (needs_commonjs_export || !came_from_cjs).then_some((name, &self.resolved_exports[name]))
      },
    )
  }

  pub fn is_canonical_exports_empty(&self) -> bool {
    self.sorted_and_non_ambiguous_resolved_exports.is_empty()
  }

  /// Whether the finalizer may fold the keys of `module`, the JSON module this metadata belongs
  /// to, into its default-export object. A namespace object needs every key as a binding.
  pub fn inlines_json_keys(&self, module: &NormalModule) -> bool {
    matches!(module.module_type, ModuleType::Json)
      && !module.exports_kind.is_commonjs()
      && !self.namespace_included
  }

  /// Whether `symbol_ref` is a key of JSON `module` that the finalizer folds into the
  /// default-export object. Such a key has no `var`, so no chunk can import or export it.
  pub fn is_inlined_json_key(
    &self,
    module: &NormalModule,
    symbols: &SymbolRefDb,
    symbol_ref: SymbolRef,
  ) -> bool {
    self.inlines_json_keys(module)
      // Only the keys are real bindings. The namespace, the default export, and the wrapper are
      // facade symbols, and the finalizer keeps them.
      && !symbols.is_facade_symbol(symbol_ref)
      && module
        .json_module_none_self_reference_included_symbol
        .as_deref()
        .is_some_and(|kept_keys| !kept_keys.contains(&symbol_ref))
  }

  #[inline]
  pub fn wrap_kind(&self) -> WrapKind {
    self.wrap_kind
  }

  #[inline]
  pub fn set_wrap_kind(&mut self, wrap_kind: WrapKind) {
    self.wrap_kind = wrap_kind;
  }

  /// Whether the namespace-object declaration will emit a `__reExport(ns, <external>)` call for
  /// this `export * from <external>` record when the namespace is rendered.
  ///
  /// This is the single source of truth for the emission decision: the module finalizer emits
  /// the call through it, and any pass that needs to predict the emission must call it instead
  /// of re-deriving the condition. In ESM output an entry-level external star
  /// re-export is flattened to a chunk-level `export * from '<external>'` statement instead, so
  /// no runtime call is needed — unless the namespace object is genuinely observed
  /// ([`ModuleNamespaceIncludedReason::Unknown`]), in which case the namespace must still merge
  /// the external's exports at runtime.
  pub fn ns_star_external_re_export_emitted(
    &self,
    rec_meta: ImportRecordMeta,
    format: OutputFormat,
  ) -> bool {
    match format {
      OutputFormat::Esm => {
        !rec_meta.contains(ImportRecordMeta::EntryLevelExternal)
          || self.module_namespace_included_reason.contains(ModuleNamespaceIncludedReason::Unknown)
      }
      OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => true,
    }
  }

  /// Export names observable through known dynamic imports, unless another consumer needs
  /// the complete namespace. See internal-docs/code-splitting/implementation.md.
  pub fn dynamically_read_export_names<'a>(
    &self,
    usage: Option<&'a DynamicImportExportsUsage>,
  ) -> Option<&'a FxHashSet<CompactStr>> {
    if self.module_namespace_included_reason.intersects(
      ModuleNamespaceIncludedReason::Unknown
        | ModuleNamespaceIncludedReason::ReExportDynamicExports,
    ) {
      return None;
    }
    match usage? {
      DynamicImportExportsUsage::Complete => None,
      DynamicImportExportsUsage::Partial(names) => Some(names),
      DynamicImportExportsUsage::Single(_) => unreachable!(),
    }
  }

  pub fn referenced_canonical_exports_symbols<'b, 'a: 'b>(
    &'b self,
    module_idx: ModuleIdx,
    entry_point_kind: EntryPointKind,
    dynamic_import_exports_usage_map: &'a FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
    needs_commonjs_export: bool,
  ) -> impl Iterator<Item = (&'b CompactStr, &'b ResolvedExport)> + 'b {
    let partial_used_exports = match entry_point_kind {
      rolldown_common::EntryPointKind::UserDefined
      | rolldown_common::EntryPointKind::EmittedUserDefined => None,
      rolldown_common::EntryPointKind::DynamicImport => {
        dynamic_import_exports_usage_map.get(&module_idx).and_then(|usage| match usage {
          DynamicImportExportsUsage::Complete => None,
          DynamicImportExportsUsage::Partial(set) => Some(set),
          DynamicImportExportsUsage::Single(_) => unreachable!(),
        })
      }
    };
    self.canonical_exports(needs_commonjs_export).filter(
      move |(name, _)| match partial_used_exports {
        Some(set) => set.contains(name.as_str()),
        None => true,
      },
    )
  }
}

pub type LinkingMetadataVec = IndexVec<ModuleIdx, LinkingMetadata>;

/// Extracts inclusion information from all module metas.
///
/// # Warning
/// This function uses `mem::take` to move `stmt_info_included` out of each meta,
/// leaving it with its default value. `is_included` and `module_namespace_included_reason`
/// are copied but not reset.
pub fn linking_metadata_vec_to_included_info(
  metas: &mut LinkingMetadataVec,
) -> (StmtInclusionVec, ModuleInclusionVec, ModuleNamespaceReasonVec) {
  let stmt_info_included_vec: StmtInclusionVec =
    metas.iter_mut().map(|meta| std::mem::take(&mut meta.stmt_info_included)).collect();

  let mut module_included_vec: ModuleInclusionVec = IndexBitSet::new(metas.len());
  for (idx, meta) in metas.iter_enumerated() {
    if meta.is_included {
      module_included_vec.set_bit(idx);
    }
  }

  let module_namespace_reason_vec: ModuleNamespaceReasonVec =
    metas.iter().map(|meta| meta.module_namespace_included_reason).collect();

  (stmt_info_included_vec, module_included_vec, module_namespace_reason_vec)
}

/// Restores inclusion information back to the module metas.
///
/// This is the reverse operation of `linking_metadata_vec_to_included_info`.
/// It should be called after modifications are done to restore the taken data.
pub fn included_info_to_linking_metadata_vec(
  metas: &mut LinkingMetadataVec,
  mut stmt_info_included_vec: StmtInclusionVec,
  module_included_vec: &ModuleInclusionVec,
  module_namespace_reason_vec: &ModuleNamespaceReasonVec,
) {
  for (idx, meta) in metas.iter_mut_enumerated() {
    meta.stmt_info_included = std::mem::take(&mut stmt_info_included_vec[idx]);
    meta.is_included = module_included_vec.has_bit(idx);
    meta.module_namespace_included_reason = module_namespace_reason_vec[idx];
  }
}
