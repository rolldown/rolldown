use rolldown_common::{ModuleIdx, SymbolRef, WrapKind};
use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub(crate) struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
}

impl OrderWrapState {
  pub(crate) fn is_empty(&self) -> bool {
    self.modules.is_empty()
  }

  pub(crate) fn esm_init_target(
    &self,
    module_idx: ModuleIdx,
    meta: &crate::types::linking_metadata::LinkingMetadata,
  ) -> Option<EsmInitTarget> {
    if matches!(meta.wrap_kind(), WrapKind::Esm) {
      return meta.wrapper_ref.map(|wrapper_ref| EsmInitTarget {
        wrapper_ref,
        init_is_noop: meta.init_is_noop,
        tla_tainted: meta.is_tla_or_contains_tla_dependency,
        origin: EsmInitOrigin::Interop,
      });
    }

    self.modules.get(&module_idx).map(|module| EsmInitTarget {
      wrapper_ref: module.wrapper_ref,
      init_is_noop: module.init_is_noop,
      tla_tainted: meta.is_tla_or_contains_tla_dependency,
      origin: EsmInitOrigin::ExecutionOrder,
    })
  }
}

#[derive(Debug)]
pub(crate) struct OrderWrappedModule {
  pub(crate) wrapper_ref: SymbolRef,
  pub(crate) init_is_noop: bool,
}

impl OrderWrappedModule {
  #[cfg(test)]
  fn new(wrapper_ref: SymbolRef) -> Self {
    Self { wrapper_ref, init_is_noop: false }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EsmInitOrigin {
  Interop,
  ExecutionOrder,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EsmInitTarget {
  pub(crate) wrapper_ref: SymbolRef,
  pub(crate) init_is_noop: bool,
  pub(crate) tla_tainted: bool,
  pub(crate) origin: EsmInitOrigin,
}

#[cfg(test)]
mod tests {
  use oxc::semantic::SymbolId;
  use rolldown_common::{ModuleIdx, SymbolRef, WrapKind};

  use super::{EsmInitOrigin, OrderWrapState, OrderWrappedModule};
  use crate::types::linking_metadata::LinkingMetadata;

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

    state.modules.insert(module_idx, OrderWrappedModule::new(wrapper_ref));

    assert_eq!(state.modules.get(&module_idx).map(|module| module.wrapper_ref), Some(wrapper_ref));
    assert_eq!(state.modules.len(), 1);
  }

  #[test]
  fn interop_esm_target_takes_precedence_over_order_state() {
    let module_idx = ModuleIdx::new(7);
    let interop_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let order_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let mut meta = LinkingMetadata::default();
    meta.sync_wrap_kind(WrapKind::Esm);
    meta.wrapper_ref = Some(interop_ref);
    let mut state = OrderWrapState::default();
    state.modules.insert(module_idx, OrderWrappedModule::new(order_ref));

    let target = state.esm_init_target(module_idx, &meta).unwrap();

    assert_eq!(target.origin, EsmInitOrigin::Interop);
    assert_eq!(target.wrapper_ref, interop_ref);
  }

  #[test]
  fn order_target_is_visible_when_interop_kind_is_none() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let mut meta = LinkingMetadata::default();
    meta.is_tla_or_contains_tla_dependency = true;
    let mut state = OrderWrapState::default();
    state.modules.insert(module_idx, OrderWrappedModule::new(wrapper_ref));

    let target = state.esm_init_target(module_idx, &meta).unwrap();

    assert_eq!(target.origin, EsmInitOrigin::ExecutionOrder);
    assert_eq!(target.wrapper_ref, wrapper_ref);
    assert!(!target.init_is_noop);
    assert!(target.tla_tainted);
  }
}
