use crate::stages::link_stage::{ModuleInclusionVec, ModuleNamespaceReasonVec, StmtInclusionVec};
use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  ConcatenateWrappedModuleKind, EntryPointKind, ImportRecordIdx, ImportRecordMeta,
  MemberExprRefResolutionMap, ModuleIdx, ModuleNamespaceIncludedReason, OutputFormat,
  ResolvedExport, RuntimeHelper, StmtInfoIdx, SymbolRef, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::IndexBitSet;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

/// The interop ESM wrapper a wrapped (`WrapKind::Esm`) module exposes: the `init_*()` binding the
/// finalizer emits its call sites against, plus whether calling it is a no-op.
///
/// Extracted so wrapper declaration emission and `init_*()` call sites read the same view of
/// [`LinkingMetadata`] instead of reaching into the raw fields independently. This keeps a single
/// place for later strict-execution-order wrapper paths to extend.
#[derive(Clone, Copy, Debug)]
pub struct EsmInitTarget {
  pub(crate) wrapper_ref: SymbolRef,
  pub(crate) init_is_noop: bool,
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
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: MemberExprRefResolutionMap,
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
  /// Set for a standalone wrapped (`WrapKind::Esm`) module whose `__esm` closure body is empty
  /// (every top-level statement is a hoisted function declaration or a source-less export clause,
  /// so nothing lands inside the wrapper closure). Calling such an `init_*` is a no-op, so init
  /// call sites are marked `@__PURE__` and the default `dce-only` minifier drops them (and the
  /// now-unused wrapper). Computed by [`crate::stages::generate_stage`]'s
  /// `compute_wrapped_esm_init_metadata`.
  pub init_is_noop: bool,
  /// For each non-included top-level re-export statement (`export * from`, `export {x} from`,
  /// `export * as ns from`) of an included `WrapKind::Esm` module: the ordered wrapped-ESM
  /// modules whose `init_*()` calls must be emitted in its place to preserve execution order.
  /// Computed by [`crate::stages::generate_stage`]'s `compute_wrapped_esm_init_metadata`;
  /// consumed by the module finalizer.
  pub transitive_esm_init_targets: FxHashMap<StmtInfoIdx, Vec<ModuleIdx>>,
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

  #[inline]
  pub fn wrap_kind(&self) -> WrapKind {
    self.wrap_kind
  }

  #[inline]
  pub fn set_wrap_kind(&mut self, wrap_kind: WrapKind) {
    self.wrap_kind = wrap_kind;
  }

  /// The wrapped-ESM init target of a module, derived from its linking metadata alone: a
  /// `WrapKind::Esm` module with an allocated wrapper symbol exposes an `init_*()` the finalizer
  /// emits; anything else has none.
  pub fn esm_init_target(&self) -> Option<EsmInitTarget> {
    if !matches!(self.wrap_kind(), WrapKind::Esm) {
      return None;
    }
    self
      .wrapper_ref
      .map(|wrapper_ref| EsmInitTarget { wrapper_ref, init_is_noop: self.init_is_noop })
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
