use std::sync::Arc;

use index_vec::IndexVec;
use oxc::{
  semantic::SymbolId,
  span::{Atom, Span},
};
use rolldown_common::{
  ExportsKind, ImportRecord, ImportRecordId, LocalExport, ModuleId, ModuleType, NamedImport,
  ResourceId, StmtInfos, SymbolRef,
};
use rustc_hash::FxHashMap;

use crate::bundler::utils::ast_scope::AstScope;

use super::NormalModule;

#[derive(Debug, Default)]
pub struct NormalModuleBuilder {
  pub id: Option<ModuleId>,
  pub source: Option<Arc<str>>,
  pub repr_name: Option<String>,
  pub path: Option<ResourceId>,
  pub named_imports: Option<FxHashMap<SymbolId, NamedImport>>,
  pub named_exports: Option<FxHashMap<Atom, LocalExport>>,
  pub stmt_infos: Option<StmtInfos>,
  pub import_records: Option<IndexVec<ImportRecordId, ImportRecord>>,
  pub imports: Option<FxHashMap<Span, ImportRecordId>>,
  pub star_exports: Option<Vec<ImportRecordId>>,
  pub scope: Option<AstScope>,
  pub default_export_ref: Option<SymbolRef>,
  pub namespace_symbol: Option<SymbolRef>,
  pub exports_kind: Option<ExportsKind>,
  pub module_type: ModuleType,
  pub is_user_defined_entry: Option<bool>,
  pub pretty_path: Option<String>,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
}

impl NormalModuleBuilder {
  pub fn build(self) -> NormalModule {
    NormalModule {
      exec_order: u32::MAX,
      id: self.id.unwrap(),
      source: self.source.unwrap(),
      repr_name: self.repr_name.unwrap(),
      resource_id: self.path.unwrap(),
      named_imports: self.named_imports.unwrap(),
      named_exports: self.named_exports.unwrap(),
      stmt_infos: self.stmt_infos.unwrap(),
      import_records: self.import_records.unwrap(),
      imports: self.imports.unwrap(),
      star_exports: self.star_exports.unwrap(),
      default_export_ref: self.default_export_ref.unwrap(),
      scope: self.scope.unwrap(),
      namespace_symbol: self.namespace_symbol.unwrap(),
      exports_kind: self.exports_kind.unwrap_or(ExportsKind::Esm),
      module_type: self.module_type,
      is_user_defined_entry: self.is_user_defined_entry.unwrap(),
      pretty_path: self.pretty_path.unwrap(),
      sourcemap_chain: self.sourcemap_chain,
    }
  }
}
