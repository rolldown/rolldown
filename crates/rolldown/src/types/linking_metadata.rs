use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rolldown_common::{
  EntryPointKind, ImportRecordIdx, MemberExprRefResolutionMap, ModuleIdx,
  ModuleNamespaceIncludedReason, ResolvedExport, RuntimeHelper, StmtInfoIdx, SymbolRef, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

/// Module metadata about linking
#[derive(Debug, Default)]
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
  pub wrap_kind: WrapKind,
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<CompactStr, ResolvedExport>,
  // pub re_export_all_names: FxHashSet<CompactStr>,
  /// Store the names of exclude ambiguous resolved exports.
  /// It will be used to generate chunk exports and module namespace binding.
  /// The second element means if the export is came from commonjs module.
  pub sorted_and_non_ambiguous_resolved_exports: FxIndexMap<CompactStr, bool>,
  // If a esm module has export star from commonjs, it will be marked as ESMWithDynamicFallback at linker.
  // The unknown export name will be resolved at runtime.
  // esbuild add it to `ExportKind`, but the linker shouldn't mutate the module.
  pub has_dynamic_exports: bool,
  pub shimmed_missing_exports: FxHashMap<CompactStr, SymbolRef>,

  // Entry chunks need to generate code that doesn't belong to any module. This is the list of symbols are referenced by the
  // generated code. Tree-shaking will cares about these symbols to make sure they are not removed.
  // The second element means if the symbol is a facade symbol.
  pub referenced_symbols_by_entry_point_chunk: Vec<(SymbolRef, bool)>,

  /// The dependencies of the module. It means if you want include this module, you need to include these dependencies too.
  pub dependencies: FxIndexSet<ModuleIdx>,
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: MemberExprRefResolutionMap,
  pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
  pub is_tla_or_contains_tla_dependency: bool,
  /// Used to to track a facade binding referenced cjs module
  /// included reexport symbol from commonjs module
  pub named_import_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx>,
  pub import_record_ns_to_cjs_module: FxHashMap<SymbolRef, ModuleIdx>,
  /// Currently our symbol link system could only link one symbol to another one, but for commonjs
  /// tree shaking, when one symbol was linked it may not only link the namespace ref symbol, and
  /// also need to link the exported facade symbol.
  pub included_commonjs_export_symbol: FxHashSet<SymbolRef>,
  pub depended_runtime_helper: RuntimeHelper,
  pub module_namespace_included_reason: ModuleNamespaceIncludedReason,
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
