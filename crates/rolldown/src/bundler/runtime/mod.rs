use oxc::{semantic::SymbolId, span::Atom};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use super::module::NormalModule;

pub static RUNTIME_PATH: &str = "rolldown-runtime.js";

#[derive(Debug)]
pub struct Runtime {
  id: ModuleId,
  name_to_symbol: FxHashMap<Atom, SymbolId>,
}

impl Runtime {
  pub fn new(id: ModuleId) -> Self {
    Self { id, name_to_symbol: FxHashMap::default() }
  }

  pub fn id(&self) -> ModuleId {
    self.id
  }
  pub fn init_symbols(&mut self, runtime_module: &NormalModule) {
    self.name_to_symbol = runtime_module
      .scope
      .get_bindings(runtime_module.scope.root_scope_id())
      .clone()
      .into_iter()
      .collect();
  }

  pub fn resolve_symbol(&self, name: &Atom) -> SymbolRef {
    let symbol_id =
      self.name_to_symbol.get(name).unwrap_or_else(|| panic!("Failed to resolve symbol: {name}"));
    (self.id, *symbol_id).into()
  }
}
