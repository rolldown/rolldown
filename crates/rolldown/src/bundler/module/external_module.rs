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
  pub exported_name_to_binding_ref: FxHashMap<Atom, SymbolRef>,
  // FIXME: make this non-optional
  pub namespace_ref: Option<SymbolRef>,
}

impl ExternalModule {
  pub fn new(id: ModuleId, resource_id: ResourceId) -> Self {
    Self {
      id,
      exec_order: u32::MAX,
      resource_id,
      import_records: IndexVec::default(),
      exported_name_to_binding_ref: FxHashMap::default(),
      namespace_ref: None,
    }
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn add_export_symbol(&mut self, symbols: &mut Symbols, exported: Atom, is_star: bool) {
    // Don't worry about the user happen to use the same name as the name we generated for `default export` or `namespace export`.
    // Since they have different `SymbolId`, so in de-conflict phase, they will be renamed to different names.
    let symbol_ref = if is_star && self.namespace_ref.is_none() {
      self.namespace_ref = Some(symbols.create_symbol(
        self.id,
        Atom::from(format!("{}_ns", self.resource_id.generate_unique_name())),
      ));
      self.namespace_ref.unwrap()
    } else {
      *self.exported_name_to_binding_ref.entry(exported.clone()).or_insert_with_key(|exported| {
        let declared_name = if exported.as_ref() == "default" {
          Atom::from(format!("{}_default", self.resource_id.generate_unique_name()))
        } else {
          exported.clone()
        };
        symbols.create_symbol(self.id, declared_name)
      })
    };
    let symbol = symbols.get_mut(symbol_ref);
    symbol.exported_as = Some(exported.clone());
    symbol.exported_as_star = is_star;
  }

  pub fn resolve_export(&self, exported: &Atom, is_star: bool) -> SymbolRef {
    if is_star {
      self.namespace_ref.expect("should have namespace ref")
    } else {
      *self.exported_name_to_binding_ref.get(exported).expect("should have export symbol")
    }
  }
}
