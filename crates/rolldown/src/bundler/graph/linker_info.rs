use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ModuleId, NamedImport, ResolvedExport, StmtInfo, SymbolRef, WrapKind};
use rustc_hash::FxHashMap;

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
  pub resolved_exports: FxHashMap<Atom, ResolvedExport>,
  pub exclude_ambiguous_resolved_exports: Vec<Atom>,
  pub resolved_star_exports: Vec<ModuleId>,
}

impl LinkingInfo {
  pub fn exports(&self) -> impl Iterator<Item = (&Atom, &ResolvedExport)> {
    self.exclude_ambiguous_resolved_exports.iter().map(|name| (name, &self.resolved_exports[name]))
  }
}

pub type LinkingInfoVec = IndexVec<ModuleId, LinkingInfo>;
