use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ModuleId, ResolvedExport, StmtInfoId, SymbolRef, WrapKind};
use rustc_hash::FxHashMap;

use super::symbols::Symbols;

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
  pub resolved_exports: FxHashMap<Atom, ResolvedExport>,
  // Store the names of exclude ambiguous resolved exports.
  // It will be used to generate chunk exports and module namespace binding.
  sorted_and_non_ambiguous_resolved_exports: Vec<Atom>,
  // If a esm module has export star from commonjs, it will be marked as ESMWithDynamicFallback at linker.
  // The unknown export name will be resolved at runtime.
  // esbuild add it to `ExportKind`, but the linker shouldn't mutate the module.
  pub has_dynamic_exports: bool,
}

impl LinkingMetadata {
  pub fn canonical_exports(&self) -> impl Iterator<Item = (&Atom, &ResolvedExport)> {
    self
      .sorted_and_non_ambiguous_resolved_exports
      .iter()
      .map(|name| (name, &self.resolved_exports[name]))
  }

  pub fn canonical_exports_len(&self) -> usize {
    self.sorted_and_non_ambiguous_resolved_exports.len()
  }

  pub fn is_canonical_exports_empty(&self) -> bool {
    self.sorted_and_non_ambiguous_resolved_exports.is_empty()
  }

  pub fn create_exclude_ambiguous_resolved_exports(&mut self, symbols: &Symbols) {
    let mut export_names = self
      .resolved_exports
      .iter()
      .filter_map(|(name, resolved_export)| {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          // because the un-ambiguous export is already union at linking imports, so here use symbols to check
          for export in potentially_ambiguous_symbol_refs {
            if resolved_export.symbol_ref != symbols.par_canonical_ref_for(*export) {
              return None;
            }
          }
        }
        Some(name.clone())
      })
      .collect::<Vec<_>>();
    export_names.sort_unstable_by(|a, b| a.cmp(b));
    self.sorted_and_non_ambiguous_resolved_exports = export_names;
  }
}

pub type LinkingMetadataVec = IndexVec<ModuleId, LinkingMetadata>;
