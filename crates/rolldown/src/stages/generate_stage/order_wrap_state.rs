use rolldown_common::{ModuleIdx, SymbolRef};
use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub(crate) struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, SymbolRef>,
}

impl OrderWrapState {
  pub(crate) fn is_empty(&self) -> bool {
    self.modules.is_empty()
  }
}

#[cfg(test)]
mod tests {
  use oxc::semantic::SymbolId;
  use rolldown_common::{ModuleIdx, SymbolRef};

  use super::OrderWrapState;

  #[test]
  fn empty_state_is_sparse() {
    let state = OrderWrapState::default();
    assert!(state.is_empty());
    assert_eq!(state.modules.len(), 0);
  }

  #[test]
  fn module_state_is_keyed_by_selected_module() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let mut state = OrderWrapState::default();

    state.modules.insert(module_idx, wrapper_ref);

    assert_eq!(state.modules.get(&module_idx), Some(&wrapper_ref));
    assert_eq!(state.modules.len(), 1);
  }
}
