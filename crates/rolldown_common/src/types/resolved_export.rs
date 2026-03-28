use crate::SymbolRef;

#[derive(Debug, Clone)]
#[expect(clippy::box_collection)]
pub struct ResolvedExport {
  // Because create export star exports happens before linking imports, The symbols can't  determine if duplicate names from export star resolution are
  // ambiguous (point to different symbols) or not (point to the same symbol).
  // Here is a example:
  //
  //   // entry.js
  //   export * from './a'
  //   export * from './b'
  //
  //   // a.js
  //   export * from './c'
  //
  //   // b.js
  //   export {x} from './c'
  //
  //   // c.js
  //   export let x = 1, y = 2
  //
  // In this case "entry.js" should have two exports "x" and "y", neither of
  // which are ambiguous. To handle this case, ambiguity resolution will be
  // deferred to linking imports.
  pub potentially_ambiguous_symbol_refs: Option<Box<Vec<SymbolRef>>>,
  pub symbol_ref: SymbolRef,
  pub came_from_commonjs: bool,
  /// When multiple CJS sources (conditional re-exports) provide the same export name,
  /// this tracks the alternative symbols. Unlike ESM ambiguity (which is an error),
  /// CJS conflicts are expected — only one branch runs at runtime, but statically
  /// we don't know which.
  pub cjs_conflicting_symbol_refs: Option<Box<Vec<SymbolRef>>>,
}

impl ResolvedExport {
  pub fn new(symbol_ref: SymbolRef, came_from_cjs: bool) -> Self {
    Self {
      symbol_ref,
      potentially_ambiguous_symbol_refs: None,
      came_from_commonjs: came_from_cjs,
      cjs_conflicting_symbol_refs: None,
    }
  }
}
