use index_vec::IndexVec;
use oxc::{semantic::SymbolId, span::Atom};
use rolldown_common::{ModuleId, NamedImport, ResolvedExport, StmtInfo, SymbolRef, WrapKind};
use rustc_hash::FxHashMap;

use super::symbols::Symbols;

/// Store the linking info for module
#[derive(Debug, Default)]
pub struct LinkingInfo {
  // The symbol for wrapped module
  pub wrap_symbol: Option<SymbolRef>,
  pub wrap_kind: WrapKind,
  pub facade_stmt_infos: Vec<StmtInfo>,
  // Convert `export { v } from "./a"` to `import { v } from "./a"; export { v }`.
  // It is used to prepare resolved exports generation.
  pub export_from_map: FxHashMap<SymbolId, NamedImport>,
  // Store the local symbol for esm export cjs at entry. eg. `export { value } from 'cjs'` => `let value = import_cjs.value; export { value };`
  pub cjs_export_symbols: FxHashMap<Atom, SymbolRef>,
  // Store the export info for each module, including export named declaration and export star declaration.
  pub resolved_exports: FxHashMap<Atom, ResolvedExport>,
  // Store the names of exclude ambiguous resolved exports.
  // It will be used to generate chunk exports and module namespace binding.
  pub exclude_ambiguous_sorted_resolved_exports: Vec<Atom>,
  pub resolved_star_exports: Vec<ModuleId>,
  // If a esm module has export star from commonjs, it will be marked as ESMWithDynamicFallback at linker.
  // The unknown export name will be resolved at runtime.
  // esbuild add it to `ExportKind`, but the linker shouldn't mutate the module.
  pub has_dynamic_exports: bool,
  // Store the local symbol for esm import cjs. eg. `var import_ns = __toESM(require_cjs())`
  pub local_symbol_for_import_cjs: FxHashMap<ModuleId, SymbolRef>,
}

impl LinkingInfo {
  pub fn sorted_exports(&self) -> impl Iterator<Item = (&Atom, &ResolvedExport)> {
    self
      .exclude_ambiguous_sorted_resolved_exports
      .iter()
      .map(|name| (name, &self.resolved_exports[name]))
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
    self.exclude_ambiguous_sorted_resolved_exports = export_names;
  }

  pub fn reference_symbol_in_facade_stmt_infos(&mut self, symbol_ref: SymbolRef) {
    self.facade_stmt_infos.push(StmtInfo {
      declared_symbols: vec![],
      // Since the facade symbol is used, it should be referenced. This will be used to
      // create correct cross-chunk links
      referenced_symbols: vec![symbol_ref],
      ..Default::default()
    });
  }
}

pub type LinkingInfoVec = IndexVec<ModuleId, LinkingInfo>;
