use index_vec::IndexVec;
use oxc::{
  semantic::{ReferenceId, ScopeTree, SymbolId},
  span::Atom,
};
use rolldown_common::{
  ImportRecord, ImportRecordId, LocalOrReExport, ModuleId, NamedImport, ResourceId, StmtInfo,
  StmtInfoId, SymbolRef,
};
use rolldown_oxc::OxcProgram;
use rustc_hash::FxHashMap;

use crate::bundler::graph::symbols::SymbolMap;

use super::NormalModule;

#[derive(Debug, Default)]
pub struct ModuleBuilder {
  pub id: Option<ModuleId>,
  pub path: Option<ResourceId>,
  pub ast: Option<OxcProgram>,
  pub named_imports: Option<FxHashMap<SymbolId, NamedImport>>,
  pub named_exports: Option<FxHashMap<Atom, LocalOrReExport>>,
  pub stmt_infos: Option<IndexVec<StmtInfoId, StmtInfo>>,
  pub import_records: Option<IndexVec<ImportRecordId, ImportRecord>>,
  pub star_exports: Option<Vec<ImportRecordId>>,
  pub scope: Option<ScopeTree>,
  pub default_export_symbol: Option<SymbolId>,
  pub namespace_symbol: Option<(SymbolRef, ReferenceId)>,
}

impl ModuleBuilder {
  pub fn initialize_namespace_binding(&mut self, symbol_table: &mut SymbolMap) {
    let name = format!("{}_ns", self.path.as_ref().unwrap().generate_unique_name());
    let symbol_ref: SymbolRef = (self.id.unwrap(), symbol_table.create_symbol(name.into())).into();
    let refer = symbol_table.create_reference(Some(symbol_ref.symbol));
    self.namespace_symbol = Some((symbol_ref, refer));
  }

  pub fn build(self) -> NormalModule {
    NormalModule {
      exec_order: u32::MAX,
      id: self.id.unwrap(),
      resource_id: self.path.unwrap(),
      ast: self.ast.unwrap(),
      named_imports: self.named_imports.unwrap(),
      named_exports: self.named_exports.unwrap(),
      stmt_infos: self.stmt_infos.unwrap(),
      import_records: self.import_records.unwrap(),
      star_exports: self.star_exports.unwrap(),
      resolved_exports: Default::default(),
      resolved_star_exports: Default::default(),
      scope: self.scope.unwrap(),
      default_export_symbol: self.default_export_symbol,
      namespace_symbol: self.namespace_symbol.unwrap(),
      is_symbol_for_namespace_referenced: false,
    }
  }
}
