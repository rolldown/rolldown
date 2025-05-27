use oxc_index::IndexVec;
use rolldown_common::{
  EntryPointKind, ImportRecordIdx, MemberExprRefResolutionMap, ModuleIdx, ResolvedExport,
  StmtInfoIdx, SymbolRef, WrapKind, dynamic_import_usage::DynamicImportExportsUsage,
};
use rolldown_rstr::Rstr;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::FxHashMap;

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
  pub resolved_exports: FxHashMap<Rstr, ResolvedExport>,
  // pub re_export_all_names: FxHashSet<Rstr>,
  // Store the names of exclude ambiguous resolved exports.
  // It will be used to generate chunk exports and module namespace binding.
  pub sorted_and_non_ambiguous_resolved_exports: Vec<Rstr>,
  // If a esm module has export star from commonjs, it will be marked as ESMWithDynamicFallback at linker.
  // The unknown export name will be resolved at runtime.
  // esbuild add it to `ExportKind`, but the linker shouldn't mutate the module.
  pub has_dynamic_exports: bool,
  pub shimmed_missing_exports: FxHashMap<Rstr, SymbolRef>,

  // Entry chunks need to generate code that doesn't belong to any module. This is the list of symbols are referenced by the
  // generated code. Tree-shaking will cares about these symbols to make sure they are not removed.
  pub referenced_symbols_by_entry_point_chunk: Vec<SymbolRef>,

  /// The dependencies of the module. It means if you want include this module, you need to include these dependencies too.
  pub dependencies: FxIndexSet<ModuleIdx>,
  // `None` the member expression resolve to a ambiguous export.
  pub resolved_member_expr_refs: MemberExprRefResolutionMap,
  pub star_exports_from_external_modules: Vec<ImportRecordIdx>,
  pub safe_cjs_to_eliminate_interop_default: bool,
  pub is_tla_or_contains_tla_dependency: bool,
}

impl LinkingMetadata {
  pub fn canonical_exports(&self) -> impl Iterator<Item = (&Rstr, &ResolvedExport)> {
    self
      .sorted_and_non_ambiguous_resolved_exports
      .iter()
      .map(|name| (name, &self.resolved_exports[name]))
  }

  pub fn is_canonical_exports_empty(&self) -> bool {
    self.sorted_and_non_ambiguous_resolved_exports.is_empty()
  }

  pub fn referenced_canonical_exports_symbols<'b, 'a: 'b>(
    &'b self,
    module_idx: ModuleIdx,
    entry_point_kind: EntryPointKind,
    dynamic_import_exports_usage_map: &'a FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  ) -> impl Iterator<Item = (&'b Rstr, &'b ResolvedExport)> + 'b {
    let partial_used_exports = match entry_point_kind {
      rolldown_common::EntryPointKind::UserDefined => None,
      rolldown_common::EntryPointKind::DynamicImport => {
        dynamic_import_exports_usage_map.get(&module_idx).and_then(|usage| match usage {
          DynamicImportExportsUsage::Complete => None,
          DynamicImportExportsUsage::Partial(set) => Some(set),
          DynamicImportExportsUsage::Single(_) => unreachable!(),
        })
      }
    };
    self.canonical_exports().filter(move |(name, _)| match partial_used_exports {
      Some(set) => set.contains(name.as_str()),
      None => true,
    })
  }
}

pub type LinkingMetadataVec = IndexVec<ModuleIdx, LinkingMetadata>;
