use oxc::{semantic::SymbolId, span::Atom};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use super::graph::symbols::SymbolMap;

pub static RUNTIME_PATH: &str = "\0rolldown-runtime.js";

#[derive(Debug, Default)]
pub struct Runtime {
  pub id: ModuleId,
  pub name_to_symbol: FxHashMap<Atom, SymbolId>,
}

impl Runtime {
  pub fn init_symbols(&mut self, runtime_symbol_map: &SymbolMap) {
    // TODO: we should only storing top level symbols here.
    // But currently, I'm not sure how to get the scope info for the runtime module
    self.name_to_symbol = runtime_symbol_map
      .names
      .iter()
      .enumerate()
      .map(|(id, name)| (name.clone(), id.into()))
      .collect()
  }

  pub fn resolve_symbol(&self, name: &Atom) -> SymbolRef {
    let symbol_id = self.name_to_symbol[name];
    (self.id, symbol_id).into()
  }
}
