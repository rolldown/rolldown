use oxc::index::IndexVec;
use rolldown_common::{NormalModuleId, ResolvedExport, StmtInfoId, SymbolRef, WrapKind};
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use super::tree_shake::UsedExportsInfo;

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
  pub wrapper_stmt_info: Option<StmtInfoId>,
  pub wrap_kind: WrapKind,
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<Rstr, ResolvedExport>,
  pub used_exports_info: UsedExportsInfo,
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
}

impl LinkingMetadata {
  pub fn canonical_exports(&self) -> impl Iterator<Item = (&Rstr, &ResolvedExport)> {
    self
      .sorted_and_non_ambiguous_resolved_exports
      .iter()
      .map(|name| (name, &self.resolved_exports[name]))
  }

  pub fn used_canonical_exports(&self) -> impl Iterator<Item = (&Rstr, &ResolvedExport)> {
    self
      .sorted_and_non_ambiguous_resolved_exports
      .iter()
      .filter(|name| self.used_exports_info.used_exports.contains(name))
      .map(|name| (name, &self.resolved_exports[name]))
  }

  pub fn is_canonical_exports_empty(&self) -> bool {
    self.sorted_and_non_ambiguous_resolved_exports.is_empty()
  }
}

pub type LinkingMetadataVec = IndexVec<NormalModuleId, LinkingMetadata>;
