use oxc::{semantic::SymbolId, span::CompactStr as CompactString};
use rustc_hash::FxHashMap;

use crate::{AstScopes, ModuleId, ModuleIdx, SymbolRef};

pub const RUNTIME_MODULE_KEY: &str = "rolldown:runtime";
pub const RUNTIME_MODULE_ID: ModuleId = ModuleId::new_arc_str(arcstr::literal!(RUNTIME_MODULE_KEY));

#[derive(Debug, Clone)]
pub struct RuntimeModuleBrief {
  id: ModuleIdx,
  name_to_symbol: FxHashMap<CompactString, SymbolId>,
}

impl RuntimeModuleBrief {
  pub fn new(id: ModuleIdx, scope: &AstScopes) -> Self {
    Self {
      id,
      name_to_symbol: scope
        .scoping()
        .get_bindings(scope.scoping().root_scope_id())
        .into_iter()
        .map(|(name, &symbol_id)| (CompactString::new(name), symbol_id))
        .collect(),
    }
  }

  pub fn id(&self) -> ModuleIdx {
    self.id
  }

  pub fn resolve_symbol(&self, name: &str) -> SymbolRef {
    let symbol_id =
      self.name_to_symbol.get(name).unwrap_or_else(|| panic!("Failed to resolve symbol: {name}"));
    (self.id, *symbol_id).into()
  }

  pub fn dummy() -> Self {
    Self { id: ModuleIdx::new(0), name_to_symbol: FxHashMap::default() }
  }
}
