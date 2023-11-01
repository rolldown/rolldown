use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeTree, SymbolId},
  span::{Atom, Span},
};
use rolldown_common::{
  ExportsKind, ImportRecord, ImportRecordId, LocalOrReExport, ModuleId, ModuleType, NamedImport,
  ResourceId, StmtInfos, SymbolRef,
};
use rolldown_oxc::OxcProgram;
use rustc_hash::FxHashMap;

use super::NormalModule;

#[derive(Debug, Default)]
pub struct NormalModuleBuilder {
  pub id: Option<ModuleId>,
  pub unique_name: Option<String>,
  pub path: Option<ResourceId>,
  pub ast: Option<OxcProgram>,
  pub named_imports: Option<FxHashMap<SymbolId, NamedImport>>,
  pub named_exports: Option<FxHashMap<Atom, LocalOrReExport>>,
  pub stmt_infos: Option<StmtInfos>,
  pub import_records: Option<IndexVec<ImportRecordId, ImportRecord>>,
  pub imports: Option<FxHashMap<Span, ImportRecordId>>,
  pub star_exports: Option<Vec<ImportRecordId>>,
  pub scope: Option<ScopeTree>,
  pub default_export_symbol: Option<SymbolId>,
  pub namespace_symbol: Option<SymbolRef>,
  pub exports_kind: Option<ExportsKind>,
  pub module_type: ModuleType,
  pub is_entry: bool,
}

impl NormalModuleBuilder {
  pub fn build(self) -> NormalModule {
    NormalModule {
      exec_order: u32::MAX,
      id: self.id.unwrap(),
      unique_name: self.unique_name.unwrap(),
      resource_id: self.path.unwrap(),
      ast: self.ast.unwrap(),
      named_imports: self.named_imports.unwrap(),
      named_exports: self.named_exports.unwrap(),
      stmt_infos: self.stmt_infos.unwrap(),
      import_records: self.import_records.unwrap(),
      imports: self.imports.unwrap(),
      star_exports: self.star_exports.unwrap(),
      default_export_symbol: self.default_export_symbol,
      scope: self.scope.unwrap(),
      namespace_symbol: self.namespace_symbol.unwrap(),
      exports_kind: self.exports_kind.unwrap_or(ExportsKind::Esm),
      module_type: self.module_type,
      is_entry: self.is_entry,
    }
  }
}
