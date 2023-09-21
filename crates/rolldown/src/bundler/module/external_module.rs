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
  pub namespace_name: Atom,
}

impl ExternalModule {
  pub fn new(id: ModuleId, resource_id: ResourceId) -> Self {
    let namespace_name = format!("{}_ns", resource_id.generate_unique_name()).into();
    Self {
      id,
      exec_order: u32::MAX,
      resource_id,
      import_records: Default::default(),
      is_symbol_for_namespace_referenced: false,
      symbols_imported_by_others: Default::default(),
      namespace_name,
    }
  }

  pub fn add_export_symbol(&mut self, symbols: &mut Symbols, exported: Atom, is_star: bool) {
    let symbol = if is_star {
      &self.namespace_name
    } else {
      &exported
    };
    self
      .symbols_imported_by_others
      .entry(symbol.clone())
      .or_insert_with(|| {
        (
          self.id,
          symbols.tables[self.id].create_symbol(symbol.clone()),
        )
          .into()
      });
  }

  pub fn resolve_export(&self, exported: Atom, is_star: bool) -> SymbolRef {
    let symbol = if is_star {
      &self.namespace_name
    } else {
      &exported
    };
    *self
      .symbols_imported_by_others
      .get(symbol)
      .expect("should have export symbol")
  }
}
