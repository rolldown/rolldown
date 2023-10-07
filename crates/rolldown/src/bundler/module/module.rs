use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::bundler::graph::symbols::Symbols;

use super::{external_module::ExternalModule, NormalModule};

#[derive(Debug)]
pub enum Module {
  Normal(NormalModule),
  External(ExternalModule),
}

impl Module {
  pub fn id(&self) -> ModuleId {
    match self {
      Module::Normal(m) => m.id,
      Module::External(m) => m.id,
    }
  }

  pub fn exec_order(&self) -> u32 {
    match self {
      Module::Normal(m) => m.exec_order,
      Module::External(m) => m.exec_order,
    }
  }

  pub fn exec_order_mut(&mut self) -> &mut u32 {
    match self {
      Module::Normal(m) => &mut m.exec_order,
      Module::External(m) => &mut m.exec_order,
    }
  }

  pub fn expect_normal(&self) -> &NormalModule {
    match self {
      Module::Normal(m) => m,
      Module::External(_) => unreachable!(),
    }
  }

  pub fn expect_normal_mut(&mut self) -> &mut NormalModule {
    match self {
      Module::Normal(m) => m,
      Module::External(_) => unreachable!(),
    }
  }

  pub fn import_records(&self) -> &IndexVec<ImportRecordId, ImportRecord> {
    match self {
      Module::Normal(m) => &m.import_records,
      Module::External(m) => &m.import_records,
    }
  }

  pub fn mark_symbol_for_namespace_referenced(&mut self) {
    match self {
      Module::Normal(m) => {
        if !m.is_symbol_for_namespace_referenced {
          m.is_symbol_for_namespace_referenced = true;
          m.initialize_namespace()
        }
      }
      Module::External(m) => m.is_symbol_for_namespace_referenced = true,
    }
  }

  pub fn render(&self, ctx: ModuleRenderContext) -> Option<MagicString<'_>> {
    match self {
      Module::Normal(m) => m.render(ctx),
      Module::External(_) => None,
    }
  }
}

pub struct ModuleRenderContext<'a> {
  pub canonical_names: &'a FxHashMap<SymbolRef, Atom>,
  pub symbols: &'a Symbols,
  pub entries_chunk_final_names: &'a FxHashMap<ModuleId, String>,
}
