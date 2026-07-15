use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, StmtInfoIdx, SymbolRef};
use rustc_hash::FxHashMap;

/// Finalizer recipes for transformed JSON modules that could not take the pristine rebuild path.
///
/// `NormalizeLazyExportsPass` is the sole producer. Each recipe refers to identities owned by the
/// final `SymbolRefDb` and `StmtInfos`, so no later link pass may rebuild either table. The artifact
/// stays sparse, crosses the unchanged link/generate boundary once, and is taken out of
/// `LinkStageOutput` immediately before module finalization. The finalizer is its only consumer and
/// drops it as soon as every retained property binding has been materialized.
#[derive(Debug, Default)]
pub struct LazyJsonExportInitializers {
  modules: FxHashMap<ModuleIdx, LazyJsonModuleExportInitializers>,
}

impl LazyJsonExportInitializers {
  pub fn record(&mut self, module_idx: ModuleIdx, module: LazyJsonModuleExportInitializers) {
    if self.modules.insert(module_idx, module).is_some() {
      tracing::error!(module = module_idx.index(), "duplicate lazy JSON initializer recipe");
    }
  }

  pub fn for_module(&self, module_idx: ModuleIdx) -> Option<&LazyJsonModuleExportInitializers> {
    self.modules.get(&module_idx)
  }
}

#[derive(Debug)]
pub struct LazyJsonModuleExportInitializers {
  payload_stmt_info_idx: StmtInfoIdx,
  entries: Box<[LazyJsonExportInitializer]>,
}

impl LazyJsonModuleExportInitializers {
  pub fn new(
    payload_stmt_info_idx: StmtInfoIdx,
    entries: Box<[LazyJsonExportInitializer]>,
  ) -> Self {
    Self { payload_stmt_info_idx, entries }
  }

  pub fn payload_stmt_info_idx(&self) -> StmtInfoIdx {
    self.payload_stmt_info_idx
  }

  pub fn entries(&self) -> &[LazyJsonExportInitializer] {
    &self.entries
  }
}

#[derive(Debug)]
pub struct LazyJsonExportInitializer {
  initializer_stmt_info_idx: StmtInfoIdx,
  binding_ref: SymbolRef,
  property_name: CompactStr,
}

impl LazyJsonExportInitializer {
  pub fn new(
    initializer_stmt_info_idx: StmtInfoIdx,
    binding_ref: SymbolRef,
    property_name: CompactStr,
  ) -> Self {
    Self { initializer_stmt_info_idx, binding_ref, property_name }
  }

  pub fn initializer_stmt_info_idx(&self) -> StmtInfoIdx {
    self.initializer_stmt_info_idx
  }

  pub fn binding_ref(&self) -> SymbolRef {
    self.binding_ref
  }

  pub fn property_name(&self) -> &str {
    &self.property_name
  }
}
