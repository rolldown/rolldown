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
  graph::symbols::{get_reference_final_name, get_symbol_final_name, Symbols},
  module::{module_id::ModuleVec, NormalModule},
};

pub struct RendererContext<'ast> {
  symbols: &'ast Symbols,
  final_names: &'ast FxHashMap<SymbolRef, Atom>,
  source: &'ast mut MagicString<'static>,
  module_to_chunk: &'ast IndexVec<ModuleId, Option<ChunkId>>,
  chunks: &'ast IndexVec<ChunkId, Chunk>,
  modules: &'ast ModuleVec,
  module: &'ast NormalModule,
  wrap_symbol_name: Option<&'ast Atom>,
  namespace_symbol_name: Option<&'ast Atom>,
  default_symbol_name: Option<&'ast Atom>,
}

impl<'ast> RendererContext<'ast> {
  pub fn new(
    symbols: &'ast Symbols,
    final_names: &'ast FxHashMap<SymbolRef, Atom>,
    source: &'ast mut MagicString<'static>,
    module_to_chunk: &'ast IndexVec<ModuleId, Option<ChunkId>>,
    chunks: &'ast IndexVec<ChunkId, Chunk>,
    modules: &'ast ModuleVec,
    module: &'ast NormalModule,
  ) -> Self {
    let wrap_symbol_name = module
      .wrap_symbol
      .and_then(|s| get_symbol_final_name(module.id, s, symbols, final_names));
    let namespace_symbol_name = get_symbol_final_name(
      module.id,
      module.namespace_symbol.0.symbol,
      symbols,
      final_names,
    );
    let default_symbol_name = module
      .default_export_symbol
      .and_then(|s| get_symbol_final_name(module.id, s, symbols, final_names));
    Self {
      symbols,
      final_names,
      source,
      module_to_chunk,
      chunks,
      modules,
      module,
      wrap_symbol_name,
      namespace_symbol_name,
      default_symbol_name,
    }
  }

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

  pub fn get_symbol_final_name(
    &self,
    module_id: ModuleId,
    symbol_id: SymbolId,
  ) -> Option<&'ast Atom> {
    get_symbol_final_name(module_id, symbol_id, self.symbols, self.final_names)
  }

  pub fn get_reference_final_name(
    &self,
    module_id: ModuleId,
    reference_id: ReferenceId,
  ) -> Option<&Atom> {
    get_reference_final_name(module_id, reference_id, self.symbols, self.final_names)
  }
}
