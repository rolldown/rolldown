use oxc::{semantic::SymbolId, span::Atom};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use super::module::NormalModule;

pub static RUNTIME_PATH: &str = "\0rolldown-runtime.js";

#[derive(Debug, Default)]
pub struct Runtime {
  pub id: ModuleId,
  pub name_to_symbol: FxHashMap<Atom, SymbolId>,
}

impl Runtime {
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
