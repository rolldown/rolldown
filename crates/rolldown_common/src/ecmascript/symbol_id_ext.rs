use oxc::semantic::SymbolId;

use crate::{ModuleIdx, SymbolRef};

pub trait SymbolIdExt {
  /// Returns true if the symbol id is the special module namespace symbol.
  fn is_module_namespace(&self) -> bool;

  fn module_namespace_symbol_ref(module_idx: ModuleIdx) -> SymbolRef;
}

impl SymbolIdExt for SymbolId {
  /// Returns true if the symbol id is the special module namespace symbol.
  /// `namespace_object_ref` is always created in second times with `create_facade_root_symbol_ref`,
  ///  see: https://github.com/rolldown/rolldown/blob/8bc7dca5a09047b6b494e3fa7b6b7564aa465372/crates/rolldown/src/ast_scanner/mod.rs?plain=1#L156-L160
  #[inline]
  fn is_module_namespace(&self) -> bool {
    *self == SymbolId::from_raw_unchecked(u32::MAX - 2)
  }

  fn module_namespace_symbol_ref(module_idx: ModuleIdx) -> SymbolRef {
    (module_idx, SymbolId::from_raw_unchecked(u32::MAX - 2)).into()
  }
}
