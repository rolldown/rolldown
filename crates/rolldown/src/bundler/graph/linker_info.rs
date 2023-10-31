use index_vec::IndexVec;
use oxc::span::Atom;
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
  pub export_from_map: FxHashMap<Atom, NamedImport>,
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
}

impl LinkingInfo {
  pub fn exports(&self) -> impl Iterator<Item = (&Atom, &ResolvedExport)> {
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
        if let Some(v) = &resolved_export.potentially_ambiguous_symbol_refs {
          if is_ambiguous_export(resolved_export.symbol_ref, v, symbols) {
            return None;
          }
        }
        Some(name.clone())
      })
      .collect::<Vec<_>>();
    export_names.sort_unstable_by(|a, b| a.cmp(b));
    self.exclude_ambiguous_sorted_resolved_exports = export_names;
  }
}

pub fn is_ambiguous_export(
  symbol_ref: SymbolRef,
  potentially_ambiguous_export: &Vec<SymbolRef>,
  symbols: &Symbols,
) -> bool {
  for export in potentially_ambiguous_export {
    if symbol_ref != symbols.par_canonical_ref_for(*export) {
      return true;
    }
  }
  false
}

pub type LinkingInfoVec = IndexVec<ModuleId, LinkingInfo>;
