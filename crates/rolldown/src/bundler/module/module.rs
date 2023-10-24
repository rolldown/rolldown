use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::bundler::{
  chunk::{ChunkId, ChunksVec},
  graph::{graph::Graph, linker::LinkerModule},
};

use super::{external_module::ExternalModule, NormalModule};

#[derive(Debug)]
pub enum Module {
  Normal(NormalModule),
  External(ExternalModule),
}

impl Module {
  pub fn id(&self) -> ModuleId {
    match self {
      Self::Normal(m) => m.id,
      Self::External(m) => m.id,
    }
  }

  pub fn exec_order(&self) -> u32 {
    match self {
      Self::Normal(m) => m.exec_order,
      Self::External(m) => m.exec_order,
    }
  }

  pub fn exec_order_mut(&mut self) -> &mut u32 {
    match self {
      Self::Normal(m) => &mut m.exec_order,
      Self::External(m) => &mut m.exec_order,
    }
  }

  pub fn _expect_normal(&self) -> &NormalModule {
    match self {
      Self::Normal(m) => m,
      Self::External(_) => unreachable!(),
    }
  }

  pub fn _expect_normal_mut(&mut self) -> &mut NormalModule {
    match self {
      Self::Normal(m) => m,
      Self::External(_) => unreachable!(),
    }
  }

  pub fn import_records(&self) -> &IndexVec<ImportRecordId, ImportRecord> {
    match self {
      Self::Normal(m) => &m.import_records,
      Self::External(m) => &m.import_records,
    }
  }

  pub fn mark_symbol_for_namespace_referenced(&self, linker_module: &mut LinkerModule) {
    match self {
      Self::Normal(m) => m.initialize_namespace(linker_module),
      Self::External(_) => linker_module.is_symbol_for_namespace_referenced = true,
    }
  }

  pub fn render(&self, ctx: ModuleRenderContext) -> Option<MagicString<'_>> {
    match self {
      Self::Normal(m) => m.render(ctx),
      Self::External(_) => None,
    }
  }
}

pub struct ModuleRenderContext<'a> {
  pub canonical_names: &'a FxHashMap<SymbolRef, Atom>,
  pub graph: &'a Graph,
  pub module_to_chunk: &'a IndexVec<ModuleId, Option<ChunkId>>,
  pub chunks: &'a ChunksVec,
}
