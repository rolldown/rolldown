/// Rolldown-specific per-module symbol metadata that lives outside the generic graph.
///
/// Contains symbol data not needed for generic linking: AST scopes,
/// symbol flags, namespace aliases, chunk assignment, etc.
///
/// During Phase 3, fields will be migrated here from Rolldown's `SymbolRefDbForModule`.
#[derive(Debug, Default, Clone)]
pub struct SymbolExtrasForModule {
  // Phase 3 will populate this with fields from SymbolRefDbForModule:
  // - AstScopes
  // - flags: FxHashMap<SymbolId, SymbolRefFlags>
  // - namespace_alias (from SymbolRefDataClassic)
  // - chunk_idx (from SymbolRefDataClassic)
  // - facade-symbol bookkeeping
  // - debug-only metadata
}
