use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, ResourceId, SymbolRef};
use rustc_hash::FxHashMap;

use crate::bundler::graph::symbols::Symbols;

#[derive(Debug)]
pub struct ExternalModule {
  pub id: ModuleId,
  pub exec_order: u32,
  pub resource_id: ResourceId,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  pub is_symbol_for_namespace_referenced: bool,
  pub symbols_imported_by_others: FxHashMap<Atom, SymbolRef>,
}

impl ExternalModule {
  pub fn new(id: ModuleId, resource_id: ResourceId) -> Self {
    Self {
      id,
      exec_order: u32::MAX,
      resource_id,
      import_records: Default::default(),
      is_symbol_for_namespace_referenced: false,
      symbols_imported_by_others: Default::default(),
    }
  }

  pub fn resolve_export(&mut self, symbols: &mut Symbols, exported: Atom) -> SymbolRef {
    *self
      .symbols_imported_by_others
      .entry(exported.clone())
      .or_insert_with(|| {
        (
          self.id,
          symbols.tables[self.id].create_symbol(exported.clone()),
        )
          .into()
      })
  }
}
