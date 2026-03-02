//! Shared conversion helpers between `oxc_module_graph` types and Rolldown types.

use rolldown_common::{ExportsKind, ModuleIdx, SymbolRef, WrapKind};

pub fn from_oxc_module_idx(idx: oxc_module_graph::types::ModuleIdx) -> ModuleIdx {
  ModuleIdx::from_usize(idx.index())
}

pub fn from_oxc_symbol_ref(sr: oxc_module_graph::types::SymbolRef) -> SymbolRef {
  SymbolRef { owner: from_oxc_module_idx(sr.owner), symbol: sr.symbol }
}

pub fn to_oxc_module_idx(idx: ModuleIdx) -> oxc_module_graph::types::ModuleIdx {
  oxc_module_graph::types::ModuleIdx::from_usize(idx.index())
}

pub fn to_oxc_symbol_ref(sr: SymbolRef) -> oxc_module_graph::types::SymbolRef {
  oxc_module_graph::types::SymbolRef::new(to_oxc_module_idx(sr.owner), sr.symbol)
}

pub fn from_oxc_wrap_kind(kind: oxc_module_graph::types::WrapKind) -> WrapKind {
  match kind {
    oxc_module_graph::types::WrapKind::None => WrapKind::None,
    oxc_module_graph::types::WrapKind::Cjs => WrapKind::Cjs,
    oxc_module_graph::types::WrapKind::Esm => WrapKind::Esm,
  }
}

pub fn from_oxc_exports_kind(kind: oxc_module_graph::types::ExportsKind) -> ExportsKind {
  match kind {
    oxc_module_graph::types::ExportsKind::Esm => ExportsKind::Esm,
    oxc_module_graph::types::ExportsKind::CommonJs => ExportsKind::CommonJs,
    oxc_module_graph::types::ExportsKind::None => ExportsKind::None,
  }
}
