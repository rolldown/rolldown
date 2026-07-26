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
  consumed_reexport_facades: FxHashSet<SymbolRef>,
}

impl OrderWrapState {
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

  /// The chunks whose rendered output calls a runtime helper that order lowering introduced: a
  /// wrapper's synthetic `init_*` declaration renders in its assigned chunk, and an overlay's
  /// lowered import/re-export glue renders at the importer's import site. Pre-lowering helper
  /// demand is not collected here — the runtime-chunk merge proof re-scans it from chunk and
  /// statement metadata.
  pub(crate) fn runtime_helper_consumer_chunks(
    &self,
    module_to_chunk: &IndexVec<ModuleIdx, Option<ChunkIdx>>,
  ) -> FxHashSet<ChunkIdx> {
    let mut consumers = FxHashSet::default();
    for stmt in &self.synthetic_statements {
      if !stmt.runtime_helpers.is_empty()
        && let Some(chunk_idx) = stmt.chunk
      {
        consumers.insert(chunk_idx);
      }
    }
    for (key, overlay) in &self.import_overlays {
      if !overlay.runtime_helpers.is_empty()
        && let Some(chunk_idx) = module_to_chunk[key.importer]
      {
        consumers.insert(chunk_idx);
      }
    }
    consumers
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

  /// Whether `symbol_ref` is the wrapper (`init_*`) binding of an execution-order-wrapped module.
  ///
  /// Such a wrapper self-rebinds on first call (`function init_x() { return (init_x =
  /// __esmMin(cb))() }`), so every later caller must observe a *live* view of the binding to run
  /// the module body exactly once. A value snapshot of the binding taken before the first call
  /// (e.g. `exports.init_x = init_x`) would freeze the pre-rebind function and re-execute the body
  /// on every subsequent call. Cross-chunk exports of these wrappers must therefore stay live
  /// getters.
  ///
  /// Interop `WrapKind::Esm` wrappers live in [`LinkingMetadata`], not in this state, and order
  /// wrappers are only created for `WrapKind::None` modules (see `lower_order_state`), so this
  /// matches exactly the `EsmInitOrigin::ExecutionOrder` targets and never an interop wrapper.
  ///
  /// [`LinkingMetadata`]: crate::types::linking_metadata::LinkingMetadata
  pub(crate) fn is_execution_order_wrapper_ref(&self, symbol_ref: SymbolRef) -> bool {
    self.modules.get(&symbol_ref.owner).is_some_and(|module| module.wrapper_ref == symbol_ref)
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

  pub(crate) fn set_consumed_reexport_facades(&mut self, facades: FxHashSet<SymbolRef>) {
    self.consumed_reexport_facades = facades;
  }

  pub(crate) fn is_consumed_reexport_facade(&self, symbol_ref: SymbolRef) -> bool {
    self.consumed_reexport_facades.contains(&symbol_ref)
  }

  pub(crate) fn esm_init_target(
    &self,
    module_idx: ModuleIdx,
    meta: &crate::types::linking_metadata::LinkingMetadata,
  ) -> Option<EsmInitTarget> {
    if matches!(meta.wrap_kind(), WrapKind::Esm) {
      return meta.wrapper_ref.map(|wrapper_ref| EsmInitTarget {
        wrapper_ref,
        tla_tainted: meta.is_tla_or_contains_tla_dependency,
        origin: EsmInitOrigin::Interop,
      });
    }

    self.modules.get(&module_idx).map(|module| EsmInitTarget {
      wrapper_ref: module.wrapper_ref,
      tla_tainted: meta.is_tla_or_contains_tla_dependency,
      origin: EsmInitOrigin::ExecutionOrder,
    })
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
    self.insert_order_wrapped_module(module_idx, wrapper_ref, Some(wrapper_statement));
  }

  /// Record a plan member as order-wrapped for the emergent-cycle fixpoint probe without minting
  /// the synthetic `init_*` statement the real lowering renders. Edge projection reads only wrapper
  /// identity (`esm_init_target`) and chunk placement (`order_wrapper_chunk`), so the probe skips
  /// the per-round synthetic-statement and runtime-helper payload entirely.
  pub(crate) fn insert_order_wrapper_probe(
    &mut self,
    module_idx: ModuleIdx,
    wrapper_ref: SymbolRef,
  ) {
    self.insert_order_wrapped_module(module_idx, wrapper_ref, None);
  }

  fn insert_order_wrapped_module(
    &mut self,
    module_idx: ModuleIdx,
    wrapper_ref: SymbolRef,
    wrapper_statement: Option<OrderSyntheticStmtIdx>,
  ) {
    assert!(
      self
        .modules
        .insert(
          module_idx,
          OrderWrappedModule {
            wrapper_ref,
            wrapper_statement,
            chunk: None,
            reexport_init_transparent: false,
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

  pub(crate) fn assign_order_wrapper_chunk(&mut self, module_idx: ModuleIdx, chunk_idx: ChunkIdx) {
    let module = self.modules.get_mut(&module_idx).expect("order-wrapped module should exist");
    module.chunk = Some(chunk_idx);
    // On the real path the wrapper statement carries the same chunk so chunk rendering can find it;
    // a probe wrapper has no statement and only needs the chunk recorded above.
    if let Some(wrapper_statement) = module.wrapper_statement {
      self.assign_synthetic_statement_chunk(wrapper_statement, chunk_idx);
    }
  }

  /// Mark an execution-order wrapper as a routing waypoint for binding-driven re-export init.
  /// Its module has no local executable body and no unconditional execution dependency, so a
  /// consumer may route directly to the wrapped leaf definers it actually consumes instead of
  /// making this shared barrel wrapper own every retained re-export path.
  pub(crate) fn set_reexport_init_transparent(&mut self, module_idx: ModuleIdx) {
    self
      .modules
      .get_mut(&module_idx)
      .expect("order-wrapped module should exist")
      .reexport_init_transparent = true;
  }

  pub(crate) fn reexport_init_is_transparent(&self, module_idx: ModuleIdx) -> bool {
    self.modules.get(&module_idx).is_some_and(|module| module.reexport_init_transparent)
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

  pub(crate) fn order_wrapper_chunk(&self, module_idx: ModuleIdx) -> Option<ChunkIdx> {
    self.modules.get(&module_idx)?.chunk
  }

  /// The target's wrapper declaration survives in the output: its declaring statement (interop)
  /// or chunk assignment (order wrap) is retained, and the module sits in a live chunk.
  pub(crate) fn init_target_included_in_live_chunk(
    &self,
    target: &EsmInitTarget,
    meta: &crate::types::linking_metadata::LinkingMetadata,
    module_idx: ModuleIdx,
    chunk_graph: &crate::chunk_graph::ChunkGraph,
  ) -> bool {
    let declaration_is_live = match target.origin {
      EsmInitOrigin::Interop => meta
        .wrapper_stmt_info
        .is_some_and(|stmt_info_idx| meta.stmt_info_included.has_bit(stmt_info_idx)),
      EsmInitOrigin::ExecutionOrder => self
        .order_wrapper_chunk(module_idx)
        .is_some_and(|chunk_idx| chunk_graph.module_to_chunk[module_idx] == Some(chunk_idx)),
    };
    declaration_is_live && chunk_graph.module_is_in_live_chunk(module_idx)
  }

  pub(crate) fn esm_init_included_in_live_chunk(
    &self,
    meta: &crate::types::linking_metadata::LinkingMetadata,
    module_idx: ModuleIdx,
    chunk_graph: &crate::chunk_graph::ChunkGraph,
  ) -> bool {
    self.esm_init_target(module_idx, meta).is_some_and(|target| {
      self.init_target_included_in_live_chunk(&target, meta, module_idx, chunk_graph)
    })
  }

  /// A runtime statement that declares an order-required runtime symbol must stay included even
  /// when tree shaking excluded it.
  pub(crate) fn forces_runtime_stmt(
    &self,
    runtime: &RuntimeModuleBrief,
    module_idx: ModuleIdx,
    stmt_info: &rolldown_common::StmtInfo,
  ) -> bool {
    module_idx == runtime.id()
      && stmt_info
        .declared_symbols
        .iter()
        .any(|declared| self.requires_runtime_symbol(runtime, declared.inner()))
  }
}

#[derive(Debug)]
pub struct OrderWrappedModule {
  pub(crate) wrapper_ref: SymbolRef,
  /// The rendered `init_*` declaration statement, minted by the real lowering. `None` on a
  /// discovery-only probe state (the emergent-cycle fixpoint), which records wrapper identity and
  /// chunk without allocating a synthetic statement it never renders.
  pub(crate) wrapper_statement: Option<OrderSyntheticStmtIdx>,
  /// The chunk the wrapper is placed in, tracked here directly so probe states need no synthetic
  /// statement to answer `order_wrapper_chunk`. Kept in sync with the wrapper statement's chunk on
  /// the real path.
  pub(crate) chunk: Option<ChunkIdx>,
  /// This order wrapper has no module-local executable body and no unconditional execution
  /// dependency. Binding-driven consumers may therefore route through it to the leaf wrappers
  /// they consume. Side-effect-only imports still call the wrapper directly.
  pub(crate) reexport_init_transparent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EsmInitOrigin {
  Interop,
  ExecutionOrder,
}

#[derive(Clone, Copy, Debug)]
pub struct EsmInitTarget {
  pub(crate) wrapper_ref: SymbolRef,
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

#[derive(Debug, Default)]
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
    Self { retained_reexport_path, ..Self::default() }
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

    let mut overlay = Self::default();
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
