use oxc_index::IndexVec;
use rolldown_common::{ChunkIdx, ModuleIdx, RuntimeHelper, SymbolRef, TaggedSymbolRef, WrapKind};
use rustc_hash::{FxHashMap, FxHashSet};

oxc_index::define_index_type! {
  pub struct OrderSyntheticStmtIdx = u32;
}

#[derive(Debug, Default)]
pub(crate) struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
  synthetic_statements: IndexVec<OrderSyntheticStmtIdx, OrderSyntheticStmt>,
  synthetic_statements_by_chunk: FxHashMap<ChunkIdx, Vec<OrderSyntheticStmtIdx>>,
}

impl OrderWrapState {
  pub(crate) fn is_empty(&self) -> bool {
    self.modules.is_empty() && self.synthetic_statements.is_empty()
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

  #[cfg(test)]
  pub(crate) fn add_synthetic_statement(
    &mut self,
    stmt: OrderSyntheticStmt,
  ) -> OrderSyntheticStmtIdx {
    self.synthetic_statements.push(stmt)
  }

  #[cfg(test)]
  pub(crate) fn assign_synthetic_statement_chunk(
    &mut self,
    stmt_idx: OrderSyntheticStmtIdx,
    chunk_idx: ChunkIdx,
  ) {
    debug_assert!(self.synthetic_statements[stmt_idx].chunk.is_none());
    self.synthetic_statements[stmt_idx].chunk = Some(chunk_idx);
    self.synthetic_statements_by_chunk.entry(chunk_idx).or_default().push(stmt_idx);
  }

  #[cfg(test)]
  pub(crate) fn synthetic_statement(&self, stmt_idx: OrderSyntheticStmtIdx) -> &OrderSyntheticStmt {
    &self.synthetic_statements[stmt_idx]
  }

  pub(crate) fn synthetic_statements_for_chunk(
    &self,
    chunk_idx: ChunkIdx,
  ) -> impl Iterator<Item = &OrderSyntheticStmt> {
    self.synthetic_statements_by_chunk.get(&chunk_idx).into_iter().flatten().map(move |stmt_idx| {
      let stmt = &self.synthetic_statements[*stmt_idx];
      debug_assert_eq!(stmt.chunk, Some(chunk_idx));
      stmt
    })
  }

  pub(crate) fn live_symbols(
    &self,
    mut canonicalize: impl FnMut(SymbolRef) -> SymbolRef,
    mut resolve_runtime_helper: impl FnMut(RuntimeHelper) -> SymbolRef,
  ) -> FxHashSet<SymbolRef> {
    let mut live_symbols = FxHashSet::default();
    for stmt in &self.synthetic_statements {
      for declared in &stmt.declared_symbols {
        live_symbols.insert(canonicalize(declared.inner()));
      }
      for referenced in &stmt.referenced_symbols {
        live_symbols.insert(canonicalize(*referenced));
      }
      for helper in stmt.runtime_helpers {
        live_symbols.insert(canonicalize(resolve_runtime_helper(helper)));
      }
    }
    live_symbols
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

#[derive(Debug)]
pub(crate) struct OrderSyntheticStmt {
  pub(crate) owner: ModuleIdx,
  pub(crate) declared_symbols: Vec<TaggedSymbolRef>,
  pub(crate) referenced_symbols: Vec<SymbolRef>,
  pub(crate) runtime_helpers: RuntimeHelper,
  pub(crate) chunk: Option<ChunkIdx>,
}

#[cfg(test)]
mod tests {
  use oxc::semantic::SymbolId;
  use rolldown_common::{ChunkIdx, ModuleIdx, RuntimeHelper, SymbolRef, TaggedSymbolRef, WrapKind};

  use super::{EsmInitOrigin, OrderSyntheticStmt, OrderWrapState, OrderWrappedModule};
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

  #[test]
  fn synthetic_wrapper_has_one_owner_and_chunk() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let runtime_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let chunk_idx = ChunkIdx::new(3);
    let mut state = OrderWrapState::default();
    let stmt_idx = state.add_synthetic_statement(OrderSyntheticStmt {
      owner: module_idx,
      declared_symbols: vec![TaggedSymbolRef::normal(wrapper_ref)],
      referenced_symbols: vec![runtime_ref],
      runtime_helpers: RuntimeHelper::EsmMin,
      chunk: None,
    });

    state.assign_synthetic_statement_chunk(stmt_idx, chunk_idx);

    let stmt = state.synthetic_statement(stmt_idx);
    let live_symbols = state.live_symbols(
      |symbol_ref| symbol_ref,
      |helper| {
        assert_eq!(helper, RuntimeHelper::EsmMin);
        runtime_ref
      },
    );
    assert_eq!(stmt.owner, module_idx);
    assert_eq!(stmt.chunk, Some(chunk_idx));
    assert!(live_symbols.contains(&wrapper_ref));
    assert!(live_symbols.contains(&runtime_ref));
    assert_eq!(state.synthetic_statements_for_chunk(chunk_idx).count(), 1);
  }

  #[test]
  fn synthetic_only_state_is_not_empty() {
    let module_idx = ModuleIdx::new(7);
    let mut state = OrderWrapState::default();
    state.add_synthetic_statement(OrderSyntheticStmt {
      owner: module_idx,
      declared_symbols: vec![],
      referenced_symbols: vec![],
      runtime_helpers: RuntimeHelper::default(),
      chunk: None,
    });

    assert!(!state.is_empty());
  }

  #[test]
  fn live_symbols_canonicalize_synthetic_references() {
    let module_idx = ModuleIdx::new(7);
    let alias_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let canonical_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let mut state = OrderWrapState::default();
    state.add_synthetic_statement(OrderSyntheticStmt {
      owner: module_idx,
      declared_symbols: vec![],
      referenced_symbols: vec![alias_ref],
      runtime_helpers: RuntimeHelper::default(),
      chunk: None,
    });

    let live_symbols = state.live_symbols(
      |symbol_ref| if symbol_ref == alias_ref { canonical_ref } else { symbol_ref },
      |_| unreachable!(),
    );

    assert!(live_symbols.contains(&canonical_ref));
    assert!(!live_symbols.contains(&alias_ref));
  }

  #[test]
  fn live_symbols_include_resolved_runtime_helpers() {
    let module_idx = ModuleIdx::new(7);
    let runtime_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let mut state = OrderWrapState::default();
    state.add_synthetic_statement(OrderSyntheticStmt {
      owner: module_idx,
      declared_symbols: vec![],
      referenced_symbols: vec![],
      runtime_helpers: RuntimeHelper::ReExport,
      chunk: None,
    });

    let live_symbols = state.live_symbols(
      |symbol_ref| symbol_ref,
      |helper| {
        assert_eq!(helper, RuntimeHelper::ReExport);
        runtime_ref
      },
    );

    assert!(live_symbols.contains(&runtime_ref));
  }
}
