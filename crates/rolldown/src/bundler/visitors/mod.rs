pub mod commonjs_source_render;
pub mod esm_source_render;
pub mod esm_wrap_source_render;
pub mod scanner;
use index_vec::IndexVec;
use oxc::{
  semantic::{ReferenceId, SymbolId},
  span::{Atom, Span},
};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, UpdateOptions};

use super::{
  chunk::{chunk::Chunk, ChunkId},
  graph::symbols::Symbols,
  module::{module_id::ModuleVec, NormalModule},
};

pub struct RendererContext<'ast> {
  pub symbols: &'ast Symbols,
  pub final_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub source: &'ast mut MagicString<'static>,
  pub module_to_chunk: &'ast IndexVec<ModuleId, Option<ChunkId>>,
  pub chunks: &'ast IndexVec<ChunkId, Chunk>,
  pub modules: &'ast ModuleVec,
  pub module: &'ast NormalModule,
}

impl<'ast> RendererContext<'ast> {
  pub fn overwrite(&mut self, start: u32, end: u32, content: String) {
    self.source.update_with(
      start,
      end,
      content,
      UpdateOptions {
        overwrite: true,
        ..Default::default()
      },
    );
  }

  pub fn remove_node(&mut self, span: Span) {
    self.source.remove(span.start, span.end);
  }

  pub fn rename_symbol(&mut self, span: Span, name: Atom) {
    self.overwrite(span.start, span.end, name.to_string());
  }

  pub fn get_symbol_final_name(&self, module_id: ModuleId, symbol_id: SymbolId) -> Option<&Atom> {
    let symbol_ref = (module_id, symbol_id).into();
    let final_ref = self.symbols.par_get_canonical_ref(symbol_ref);
    self.final_names.get(&final_ref)
  }

  pub fn get_reference_final_name(
    &self,
    module_id: ModuleId,
    reference_id: ReferenceId,
  ) -> Option<&Atom> {
    self.symbols.tables[module_id].references[reference_id]
      .and_then(|symbol| self.get_symbol_final_name(module_id, symbol))
  }

  pub fn get_wrap_symbol_name(&self) -> Option<&Atom> {
    self
      .module
      .wrap_symbol
      .and_then(|s| self.get_symbol_final_name(self.module.id, s))
  }

  pub fn get_namespace_symbol_name(&self) -> Option<&Atom> {
    self.get_symbol_final_name(self.module.id, self.module.namespace_symbol.0.symbol)
  }

  pub fn get_default_symbol_name(&self) -> Option<&Atom> {
    self
      .module
      .default_export_symbol
      .and_then(|s| self.get_symbol_final_name(self.module.id, s))
  }
}
