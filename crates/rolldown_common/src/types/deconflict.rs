use oxc::semantic::{ScopeId, SymbolId};
use oxc_index::IndexVec;
use rustc_hash::FxHashMap;

use crate::ModuleIdx;

pub type ModuleScopeSymbolIdMap<'a> =
  FxHashMap<ModuleIdx, IndexVec<ScopeId, Vec<(SymbolId, &'a str)>>>;
