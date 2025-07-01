use crate::SymbolRef;

#[derive(Debug, Clone)]
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
  pub potentially_ambiguous_symbol_refs: Option<Vec<SymbolRef>>,
  pub symbol_ref: SymbolRef,
  pub came_from_cjs: bool,
}

impl ResolvedExport {
  pub fn new(symbol_ref: SymbolRef, came_from_cjs: bool) -> Self {
    Self { symbol_ref, potentially_ambiguous_symbol_refs: None, came_from_cjs }
  }
}
