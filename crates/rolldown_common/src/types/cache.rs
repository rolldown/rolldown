use arcstr::ArcStr;
use dashmap::mapref::one::Ref;
use oxc::semantic::{ScopeTree, SymbolTable};
use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::dashmap::FxDashMap;

use crate::{ImportRecordIdx, ModuleType, ResolvedId, StrOrBytes};

#[derive(Default)]
pub struct Cache {
  ecma_ast: FxDashMap<ArcStr, EcmaAst>,
  raw_source_and_module_type: FxDashMap<ArcStr, (StrOrBytes, ModuleType)>,
  scopes: FxDashMap<ArcStr, ScopeTree>,
  symbol_table: FxDashMap<ArcStr, SymbolTable>,
  has_lazy_export: FxDashMap<ArcStr, bool>,
  resolved_dep: FxDashMap<ArcStr, IndexVec<ImportRecordIdx, ResolvedId>>,
}

impl Cache {
  pub fn get_ecma_ast(&self, key: &str) -> Option<Ref<'_, ArcStr, EcmaAst>> {
    self.ecma_ast.get(key)
  }

  pub fn invalidate(&self, key: &str) {
    self.ecma_ast.remove(key);
    self.raw_source_and_module_type.remove(key);
  }

  pub fn get_source(&self, key: &str) -> Option<ArcStr> {
    let source = self.ecma_ast.get(key).map(|item| {
      let value = item.value();
      value.source().clone()
    });
    source
  }

  pub fn insert_ecma_ast(&self, key: ArcStr, value: EcmaAst) {
    self.ecma_ast.insert(key, value);
  }

  pub fn insert_resolved_dep(&self, key: ArcStr, value: IndexVec<ImportRecordIdx, ResolvedId>) {
    self.resolved_dep.insert(key, value);
  }

  pub fn get_resolved_dep(
    &self,
    key: &str,
  ) -> Option<Ref<'_, ArcStr, IndexVec<ImportRecordIdx, ResolvedId>>> {
    self.resolved_dep.get(key)
  }

  pub fn insert_has_lazy_export(&self, key: ArcStr, value: bool) {
    self.has_lazy_export.insert(key, value);
  }

  pub fn get_has_lazy_export(&self, key: &str) -> Option<Ref<'_, ArcStr, bool>> {
    self.has_lazy_export.get(key)
  }

  pub fn insert_scope(&self, key: ArcStr, value: ScopeTree) {
    self.scopes.insert(key, value);
  }

  pub fn insert_symbol_table(&self, key: ArcStr, value: SymbolTable) {
    self.symbol_table.insert(key, value);
  }

  pub fn get_symbol_table(&self, key: &str) -> Option<Ref<'_, ArcStr, SymbolTable>> {
    self.symbol_table.get(key)
  }

  pub fn remove_scope(&self, key: &str) -> Option<(ArcStr, ScopeTree)> {
    self.scopes.remove(key)
  }

  pub fn get_raw_source_and_module_type(
    &self,
    key: &str,
  ) -> Option<Ref<'_, ArcStr, (StrOrBytes, ModuleType)>> {
    self.raw_source_and_module_type.get(key)
  }

  pub fn insert_raw_source_and_module_type(&self, key: ArcStr, value: (StrOrBytes, ModuleType)) {
    self.raw_source_and_module_type.insert(key, value);
  }
}
