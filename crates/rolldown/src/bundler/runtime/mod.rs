use oxc::{semantic::SymbolId, span::Atom};
use rolldown_common::{NormalModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use super::types::ast_scope::AstScope;

#[derive(Debug)]
pub struct RuntimeModuleBrief {
  id: NormalModuleId,
  name_to_symbol: FxHashMap<Atom, SymbolId>,
}

impl RuntimeModuleBrief {
  pub fn new(id: NormalModuleId, scope: &AstScope) -> Self {
    Self {
      id,
      name_to_symbol: scope.get_bindings(scope.root_scope_id()).clone().into_iter().collect(),
    }
  }

  pub fn id(&self) -> NormalModuleId {
    self.id
  }

  pub fn resolve_symbol(&self, name: &str) -> SymbolRef {
    let symbol_id =
      self.name_to_symbol.get(name).unwrap_or_else(|| panic!("Failed to resolve symbol: {name}"));
    (self.id, *symbol_id).into()
  }
}
