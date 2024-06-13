use oxc::index::IndexVec;
use oxc::{
  semantic::{ScopeId, SymbolFlags, SymbolId, SymbolTable},
  span::{CompactStr as CompactString, Span},
};

#[derive(Debug, Default)]
pub struct AstSymbols {
  pub names: IndexVec<SymbolId, CompactString>,
  pub scope_ids: IndexVec<SymbolId, ScopeId>,
  pub spans: IndexVec<SymbolId, Span>,
  pub flags: IndexVec<SymbolId, SymbolFlags>,
}

impl AstSymbols {
  pub fn from_symbol_table(table: SymbolTable) -> Self {
    debug_assert!(table.references.is_empty());
    Self { names: table.names, scope_ids: table.scope_ids, spans: table.spans, flags: table.flags }
  }

  pub fn create_symbol(&mut self, name: CompactString, scope_id: ScopeId) -> SymbolId {
    self.scope_ids.push(scope_id);
    self.names.push(name)
  }

  pub fn scope_id_for(&self, symbol_id: SymbolId) -> ScopeId {
    self.scope_ids[symbol_id]
  }

  pub fn get_span(&self, symbol_id: SymbolId) -> Span {
    self.spans[symbol_id]
  }

  pub fn get_name(&self, symbol_id: SymbolId) -> &str {
    &self.names[symbol_id]
  }

  pub fn get_flag(&self, symbol_id: SymbolId) -> SymbolFlags {
    self.flags[symbol_id]
  }
}
