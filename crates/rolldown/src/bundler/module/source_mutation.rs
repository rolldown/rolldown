use oxc::{
  semantic::{ReferenceId, SymbolId},
  span::{Atom, Span},
};
use rolldown_common::{ModuleId, SymbolRef};
use rustc_hash::FxHashMap;

use crate::bundler::graph::symbols::Symbols;

#[derive(Debug)]
pub enum SourceMutation {
  RenameSymbol(Box<(Span, Atom)>),
  Remove(Box<Span>),
  AddExportDefaultBindingIdentifier(Box<Span>),
  //   AddNamespaceExport(),
}

pub fn get_symbol_final_name<'a>(
  module_id: ModuleId,
  symbol_id: SymbolId,
  symbols: &'a Symbols,
  final_names: &'a FxHashMap<SymbolRef, Atom>,
) -> Option<&'a Atom> {
  let symbol_ref = (module_id, symbol_id).into();
  let final_ref = symbols.par_get_canonical_ref(symbol_ref);
  final_names.get(&final_ref)
}

pub fn get_reference_final_name<'a>(
  module_id: ModuleId,
  reference_id: ReferenceId,
  symbols: &'a Symbols,
  final_names: &'a FxHashMap<SymbolRef, Atom>,
) -> Option<&'a Atom> {
  symbols.tables[module_id].references[reference_id]
    .and_then(|symbol| get_symbol_final_name(module_id, symbol, symbols, final_names))
}
