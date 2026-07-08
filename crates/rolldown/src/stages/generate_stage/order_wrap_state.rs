use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx, RUNTIME_HELPER_NAMES,
  RuntimeHelper, RuntimeModuleBrief, StmtInfoIdx, StmtInfos, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefDb, TaggedSymbolRef, WrapKind,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

oxc_index::define_index_type! {
  pub struct OrderSyntheticStmtIdx = u32;
}

#[derive(Debug, Default)]
pub struct OrderWrapState {
  modules: FxHashMap<ModuleIdx, OrderWrappedModule>,
  synthetic_statements: IndexVec<OrderSyntheticStmtIdx, OrderSyntheticStmt>,
  synthetic_statements_by_chunk: FxHashMap<ChunkIdx, Vec<OrderSyntheticStmtIdx>>,
  import_overlays: FxHashMap<OrderImportKey, OrderImportOverlay>,
  import_overlays_by_importer: FxHashMap<ModuleIdx, Vec<OrderImportKey>>,
  import_overlays_by_statement: FxHashMap<(ModuleIdx, StmtInfoIdx), Vec<OrderImportKey>>,
  namespace_requirements: FxHashMap<SymbolRef, FxIndexSet<ModuleIdx>>,
  runtime_symbols: FxHashSet<SymbolRef>,
  nested_reexport_records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
}

impl OrderWrapState {
  #[cfg(test)]
  pub(crate) fn is_empty(&self) -> bool {
    self.modules.is_empty()
      && self.synthetic_statements.is_empty()
      && self.import_overlays.is_empty()
      && self.namespace_requirements.is_empty()
      && self.runtime_symbols.is_empty()
      && self.nested_reexport_records.is_empty()
  }

  pub(crate) fn has_import_overlays(&self) -> bool {
    !self.import_overlays.is_empty()
  }

  pub(crate) fn required_runtime_helpers(&self) -> RuntimeHelper {
    let synthetic_helpers = self
      .synthetic_statements
      .iter()
      .fold(RuntimeHelper::default(), |helpers, stmt| helpers | stmt.runtime_helpers);
    self
      .import_overlays
      .values()
      .fold(synthetic_helpers, |helpers, overlay| helpers | overlay.runtime_helpers)
  }

  pub(crate) fn requires_runtime_symbol(
    &self,
    runtime: &RuntimeModuleBrief,
    symbol_ref: SymbolRef,
  ) -> bool {
    self.runtime_symbols.contains(&symbol_ref)
      || self.required_runtime_helpers().into_iter().any(|helper| {
        runtime.resolve_symbol(RUNTIME_HELPER_NAMES[helper.bit_index()]) == symbol_ref
      })
  }

  pub(crate) fn compute_runtime_symbol_closure(
    &mut self,
    runtime: &RuntimeModuleBrief,
    stmt_infos: &StmtInfos,
    symbols: &SymbolRefDb,
  ) {
    let mut pending = self
      .required_runtime_helpers()
      .into_iter()
      .map(|helper| runtime.resolve_symbol(RUNTIME_HELPER_NAMES[helper.bit_index()]))
      .collect::<Vec<_>>();
    while let Some(symbol_ref) = pending.pop() {
      let symbol_ref = symbols.canonical_ref_resolving_namespace(symbol_ref);
      if !self.runtime_symbols.insert(symbol_ref) {
        continue;
      }
      for stmt_idx in stmt_infos.declared_stmts_by_symbol(&symbol_ref) {
        for referenced in &stmt_infos[*stmt_idx].referenced_symbols {
          if let SymbolOrMemberExprRef::Symbol(referenced) = referenced
            && referenced.owner == runtime.id()
          {
            pending.push(*referenced);
          }
        }
      }
    }
  }

  pub(crate) fn has_order_wrapper(&self, module_idx: ModuleIdx) -> bool {
    self.modules.contains_key(&module_idx)
  }

  pub(crate) fn set_nested_reexport_records(
    &mut self,
    records: FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  ) {
    self.nested_reexport_records = records;
  }

  pub(crate) fn is_nested_reexport_record(
    &self,
    module_idx: ModuleIdx,
    rec_idx: ImportRecordIdx,
  ) -> bool {
    self.nested_reexport_records.contains(&(module_idx, rec_idx))
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

    if let Some(module) = self.modules.get(&module_idx) {
      return Some(EsmInitTarget {
        wrapper_ref: module.wrapper_ref,
        init_is_noop: module.init_is_noop,
        tla_tainted: meta.is_tla_or_contains_tla_dependency,
        origin: EsmInitOrigin::ExecutionOrder,
      });
    }

    None
  }

  pub(crate) fn insert_order_wrapper(
    &mut self,
    module_idx: ModuleIdx,
    wrapper_ref: SymbolRef,
    runtime_helper: RuntimeHelper,
  ) {
    let wrapper_statement = self.add_synthetic_statement(OrderSyntheticStmt {
      owner: module_idx,
      declared_symbols: vec![TaggedSymbolRef::normal(wrapper_ref)],
      referenced_symbols: vec![],
      runtime_helpers: runtime_helper,
      chunk: None,
    });
    assert!(
      self
        .modules
        .insert(
          module_idx,
          OrderWrappedModule {
            wrapper_ref,
            wrapper_statement,
            init_is_noop: false,
            transitive_init_targets: FxHashMap::default(),
          },
        )
        .is_none(),
      "duplicate order-wrapped module",
    );
  }

  pub(crate) fn add_synthetic_statement(
    &mut self,
    stmt: OrderSyntheticStmt,
  ) -> OrderSyntheticStmtIdx {
    self.synthetic_statements.push(stmt)
  }

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

  pub(crate) fn assign_order_wrapper_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    let wrapper_statement = self
      .modules
      .get(&module_idx)
      .map(|module| module.wrapper_statement)
      .expect("order-wrapped module should have a synthetic declaration");
    self.assign_synthetic_statement_chunk(wrapper_statement, chunk_idx);
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
    overlay_is_live: impl Fn(ModuleIdx) -> bool,
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
    for (key, overlay) in &self.import_overlays {
      if !overlay_is_live(key.importer) {
        continue;
      }
      for referenced in &overlay.referenced_symbols {
        live_symbols.insert(canonicalize(*referenced));
      }
      for helper in overlay.runtime_helpers {
        live_symbols.insert(canonicalize(resolve_runtime_helper(helper)));
      }
    }
    live_symbols.extend(self.runtime_symbols.iter().copied().map(&mut canonicalize));
    live_symbols
  }

  pub(crate) fn insert_import_overlay(
    &mut self,
    key: OrderImportKey,
    overlay: OrderImportOverlay,
    importer_namespace_ref: SymbolRef,
    importee_namespace_ref: SymbolRef,
  ) {
    if overlay.requires_importer_namespace {
      self.namespace_requirements.entry(importer_namespace_ref).or_default().insert(key.importer);
    }
    if overlay.requires_importee_namespace {
      self.namespace_requirements.entry(importee_namespace_ref).or_default().insert(key.importer);
    }
    assert!(self.import_overlays.insert(key, overlay).is_none(), "duplicate order import overlay");
    self.import_overlays_by_importer.entry(key.importer).or_default().push(key);
    self.import_overlays_by_statement.entry((key.importer, key.statement)).or_default().push(key);
  }

  pub(crate) fn import_overlay(&self, key: OrderImportKey) -> Option<&OrderImportOverlay> {
    self.import_overlays.get(&key)
  }

  pub(crate) fn import_overlays_for_importer(
    &self,
    importer_idx: ModuleIdx,
  ) -> impl Iterator<Item = (OrderImportKey, &OrderImportOverlay)> {
    self
      .import_overlays_by_importer
      .get(&importer_idx)
      .into_iter()
      .flatten()
      .filter_map(|key| self.import_overlay(*key).map(|overlay| (*key, overlay)))
  }

  pub(crate) fn import_overlays_for_statement(
    &self,
    importer_idx: ModuleIdx,
    statement: StmtInfoIdx,
  ) -> impl Iterator<Item = (OrderImportKey, &OrderImportOverlay)> {
    self
      .import_overlays_by_statement
      .get(&(importer_idx, statement))
      .into_iter()
      .flatten()
      .filter_map(|key| self.import_overlay(*key).map(|overlay| (*key, overlay)))
  }

  pub(crate) fn requires_namespace(
    &self,
    symbol_ref: SymbolRef,
    importer_is_live: impl Fn(ModuleIdx) -> bool,
  ) -> bool {
    self
      .namespace_requirements
      .get(&symbol_ref)
      .is_some_and(|importers| importers.iter().copied().any(importer_is_live))
  }

  pub(crate) fn set_order_init_metadata(
    &mut self,
    module_idx: ModuleIdx,
    init_is_noop: bool,
    transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<ModuleIdx>>,
  ) {
    let module = self.modules.get_mut(&module_idx).expect("order-wrapped module should exist");
    module.init_is_noop = init_is_noop;
    module.transitive_init_targets = transitive_init_targets;
  }

  pub(crate) fn transitive_init_targets<'a>(
    &'a self,
    module_idx: ModuleIdx,
    meta: &'a crate::types::linking_metadata::LinkingMetadata,
  ) -> &'a FxHashMap<StmtInfoIdx, Vec<ModuleIdx>> {
    if self
      .esm_init_target(module_idx, meta)
      .is_some_and(|target| matches!(target.origin, EsmInitOrigin::ExecutionOrder))
    {
      if let Some(module) = self.modules.get(&module_idx) {
        return &module.transitive_init_targets;
      }
    }
    &meta.transitive_esm_init_targets
  }

  pub(crate) fn order_wrapper_chunk(&self, module_idx: ModuleIdx) -> Option<ChunkIdx> {
    let module = self.modules.get(&module_idx)?;
    self.synthetic_statements.get(module.wrapper_statement).and_then(|stmt| stmt.chunk)
  }
}

#[derive(Debug)]
pub struct OrderWrappedModule {
  pub(crate) wrapper_ref: SymbolRef,
  pub(crate) wrapper_statement: OrderSyntheticStmtIdx,
  pub(crate) init_is_noop: bool,
  pub(crate) transitive_init_targets: FxHashMap<StmtInfoIdx, Vec<ModuleIdx>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EsmInitOrigin {
  Interop,
  ExecutionOrder,
}

#[derive(Clone, Copy, Debug)]
pub struct EsmInitTarget {
  pub(crate) wrapper_ref: SymbolRef,
  pub(crate) init_is_noop: bool,
  pub(crate) tla_tainted: bool,
  pub(crate) origin: EsmInitOrigin,
}

#[derive(Debug)]
pub struct OrderSyntheticStmt {
  pub(crate) owner: ModuleIdx,
  pub(crate) declared_symbols: Vec<TaggedSymbolRef>,
  pub(crate) referenced_symbols: Vec<SymbolRef>,
  pub(crate) runtime_helpers: RuntimeHelper,
  pub(crate) chunk: Option<ChunkIdx>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OrderImportKey {
  pub(crate) importer: ModuleIdx,
  pub(crate) statement: StmtInfoIdx,
  pub(crate) record: ImportRecordIdx,
}

#[derive(Debug)]
pub struct OrderImportOverlay {
  pub(crate) referenced_symbols: Vec<SymbolRef>,
  pub(crate) runtime_helpers: RuntimeHelper,
  pub(crate) requires_importer_namespace: bool,
  pub(crate) requires_importee_namespace: bool,
  pub(crate) reexports_dynamic_exports: bool,
  pub(crate) retained_reexport_path: Vec<(ModuleIdx, ImportRecordIdx)>,
}

impl OrderImportOverlay {
  pub(crate) fn transitive_reexport(
    retained_reexport_path: Vec<(ModuleIdx, ImportRecordIdx)>,
  ) -> Self {
    Self {
      referenced_symbols: vec![],
      runtime_helpers: RuntimeHelper::default(),
      requires_importer_namespace: false,
      requires_importee_namespace: false,
      reexports_dynamic_exports: false,
      retained_reexport_path,
    }
  }

  #[expect(clippy::too_many_arguments)]
  pub(crate) fn from_import_record(
    kind: ImportKind,
    meta: ImportRecordMeta,
    wrapper_ref: SymbolRef,
    importer_namespace_ref: SymbolRef,
    importee_namespace_ref: SymbolRef,
    importee_has_dynamic_exports: bool,
    active_execution_dependency: bool,
    code_splitting_disabled: bool,
  ) -> Option<Self> {
    let is_reexport =
      meta.intersects(ImportRecordMeta::IsExportStar | ImportRecordMeta::IsReExportOnly);
    if !active_execution_dependency && !is_reexport {
      return None;
    }

    let mut overlay = Self {
      referenced_symbols: vec![],
      runtime_helpers: RuntimeHelper::default(),
      requires_importer_namespace: false,
      requires_importee_namespace: false,
      reexports_dynamic_exports: false,
      retained_reexport_path: vec![],
    };
    let mut reference = |symbol_ref| {
      if !overlay.referenced_symbols.contains(&symbol_ref) {
        overlay.referenced_symbols.push(symbol_ref);
      }
    };

    match kind {
      ImportKind::Import => {
        reference(wrapper_ref);
        if meta.contains(ImportRecordMeta::IsExportStar) && importee_has_dynamic_exports {
          reference(importer_namespace_ref);
          reference(importee_namespace_ref);
          overlay.runtime_helpers.insert(RuntimeHelper::ReExport);
          overlay.requires_importer_namespace = true;
          overlay.requires_importee_namespace = true;
          overlay.reexports_dynamic_exports = true;
        }
      }
      ImportKind::Require => {
        reference(wrapper_ref);
        reference(importee_namespace_ref);
        overlay.requires_importee_namespace = true;
        if !meta.contains(ImportRecordMeta::IsRequireUnused) {
          overlay.runtime_helpers.insert(RuntimeHelper::ToCommonJs);
        }
      }
      ImportKind::DynamicImport if code_splitting_disabled => {
        reference(wrapper_ref);
        reference(importee_namespace_ref);
        overlay.requires_importee_namespace = true;
      }
      ImportKind::DynamicImport
      | ImportKind::AtImport
      | ImportKind::UrlImport
      | ImportKind::NewUrl
      | ImportKind::HotAccept => return None,
    }

    Some(overlay)
  }
}

#[cfg(test)]
mod tests {
  use oxc::semantic::SymbolId;
  use rolldown_common::{
    ChunkIdx, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleIdx, RuntimeHelper, StmtInfoIdx,
    SymbolRef, TaggedSymbolRef, WrapKind,
  };
  use rustc_hash::FxHashMap;

  use super::{
    EsmInitOrigin, OrderImportKey, OrderImportOverlay, OrderSyntheticStmt, OrderWrapState,
  };
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

    state.insert_order_wrapper(module_idx, wrapper_ref, RuntimeHelper::EsmMin);

    assert_eq!(state.modules.get(&module_idx).map(|module| module.wrapper_ref), Some(wrapper_ref));
    assert_eq!(state.modules.len(), 1);
    assert_eq!(state.synthetic_statements.len(), 1);
  }

  #[test]
  fn interop_esm_target_takes_precedence_over_order_state() {
    let module_idx = ModuleIdx::new(7);
    let interop_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let order_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let mut meta = LinkingMetadata::default();
    meta.set_wrap_kind(WrapKind::Esm);
    meta.wrapper_ref = Some(interop_ref);
    let mut state = OrderWrapState::default();
    state.insert_order_wrapper(module_idx, order_ref, RuntimeHelper::EsmMin);

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
    state.insert_order_wrapper(module_idx, wrapper_ref, RuntimeHelper::EsmMin);

    let target = state.esm_init_target(module_idx, &meta).unwrap();

    assert_eq!(target.origin, EsmInitOrigin::ExecutionOrder);
    assert_eq!(target.wrapper_ref, wrapper_ref);
    assert!(!target.init_is_noop);
    assert!(target.tla_tainted);
  }

  #[test]
  fn order_init_metadata_is_owned_by_order_state() {
    let module_idx = ModuleIdx::new(7);
    let target_idx = ModuleIdx::new(8);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let stmt_idx = StmtInfoIdx::new(2);
    let meta = LinkingMetadata::default();
    let mut state = OrderWrapState::default();
    state.insert_order_wrapper(module_idx, wrapper_ref, RuntimeHelper::EsmMin);

    state.set_order_init_metadata(
      module_idx,
      true,
      FxHashMap::from_iter([(stmt_idx, vec![target_idx])]),
    );

    assert!(state.esm_init_target(module_idx, &meta).unwrap().init_is_noop);
    assert_eq!(state.transitive_init_targets(module_idx, &meta)[&stmt_idx], [target_idx]);
    assert!(!meta.init_is_noop);
    assert!(meta.transitive_esm_init_targets.is_empty());
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
      |_| true,
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
      |_| true,
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
      |_| true,
    );

    assert!(live_symbols.contains(&runtime_ref));
  }

  #[test]
  fn inactive_order_import_has_no_overlay() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let importee_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(2)));

    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Import,
      ImportRecordMeta::empty(),
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      false,
      false,
      false,
    );

    assert!(overlay.is_none());
  }

  #[test]
  fn active_order_import_references_only_wrapper() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let importee_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(2)));

    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Import,
      ImportRecordMeta::empty(),
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      false,
      true,
      false,
    )
    .expect("active import should have an overlay");

    assert_eq!(overlay.referenced_symbols, [wrapper_ref]);
    assert!(overlay.runtime_helpers.is_empty());
    assert!(!overlay.requires_importer_namespace);
    assert!(!overlay.requires_importee_namespace);
  }

  #[test]
  fn export_star_overlay_records_namespaces_and_reexport_helper() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let importee_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(2)));

    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Import,
      ImportRecordMeta::IsExportStar,
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      true,
      false,
      false,
    )
    .expect("retained re-export should have an overlay");

    assert_eq!(
      overlay.referenced_symbols,
      [wrapper_ref, importer_namespace_ref, importee_namespace_ref]
    );
    assert!(overlay.runtime_helpers.contains(RuntimeHelper::ReExport));
    assert!(overlay.requires_importer_namespace);
    assert!(overlay.requires_importee_namespace);
    assert!(overlay.reexports_dynamic_exports);
  }

  #[test]
  fn require_overlay_records_namespace_and_commonjs_helper() {
    let module_idx = ModuleIdx::new(7);
    let wrapper_ref = SymbolRef::from((module_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(1)));
    let importee_namespace_ref = SymbolRef::from((module_idx, SymbolId::from_usize(2)));

    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Require,
      ImportRecordMeta::empty(),
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      false,
      true,
      false,
    )
    .expect("active require should have an overlay");

    assert_eq!(overlay.referenced_symbols, [wrapper_ref, importee_namespace_ref]);
    assert!(overlay.runtime_helpers.contains(RuntimeHelper::ToCommonJs));
    assert!(!overlay.requires_importer_namespace);
    assert!(overlay.requires_importee_namespace);
  }

  #[test]
  fn inserted_overlay_is_keyed_and_records_namespace_requirements() {
    let importer_idx = ModuleIdx::new(7);
    let importee_idx = ModuleIdx::new(8);
    let wrapper_ref = SymbolRef::from((importee_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((importer_idx, SymbolId::from_usize(0)));
    let importee_namespace_ref = SymbolRef::from((importee_idx, SymbolId::from_usize(1)));
    let key = OrderImportKey {
      importer: importer_idx,
      statement: StmtInfoIdx::new(2),
      record: ImportRecordIdx::new(3),
    };
    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Import,
      ImportRecordMeta::IsExportStar,
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      true,
      false,
      false,
    )
    .unwrap();
    let mut state = OrderWrapState::default();

    state.insert_import_overlay(key, overlay, importer_namespace_ref, importee_namespace_ref);

    assert!(state.import_overlay(key).is_some());
    assert_eq!(state.import_overlays_for_statement(importer_idx, key.statement).count(), 1);
    assert!(state.requires_namespace(importer_namespace_ref, |_| true));
    assert!(state.requires_namespace(importee_namespace_ref, |_| true));
    assert!(!state.requires_namespace(importee_namespace_ref, |_| false));
  }

  #[test]
  fn overlay_symbols_are_live_only_when_importer_is_rendered() {
    let importer_idx = ModuleIdx::new(7);
    let importee_idx = ModuleIdx::new(8);
    let wrapper_ref = SymbolRef::from((importee_idx, SymbolId::from_usize(0)));
    let importer_namespace_ref = SymbolRef::from((importer_idx, SymbolId::from_usize(0)));
    let importee_namespace_ref = SymbolRef::from((importee_idx, SymbolId::from_usize(1)));
    let key = OrderImportKey {
      importer: importer_idx,
      statement: StmtInfoIdx::new(2),
      record: ImportRecordIdx::new(3),
    };
    let overlay = OrderImportOverlay::from_import_record(
      ImportKind::Import,
      ImportRecordMeta::empty(),
      wrapper_ref,
      importer_namespace_ref,
      importee_namespace_ref,
      false,
      true,
      false,
    )
    .unwrap();
    let mut state = OrderWrapState::default();
    state.insert_import_overlay(key, overlay, importer_namespace_ref, importee_namespace_ref);

    let live_symbols = state.live_symbols(|symbol_ref| symbol_ref, |_| unreachable!(), |_| false);

    assert!(!live_symbols.contains(&wrapper_ref));
  }
}
