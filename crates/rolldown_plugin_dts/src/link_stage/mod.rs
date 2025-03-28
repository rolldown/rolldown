use std::collections::HashSet;

use dts_linking_meta_data::DtsLinkingMetadataVec;
use oxc_index::IndexVec;
use rolldown_common::{ModuleIdx, SymbolRef, SymbolRefDb};
use rolldown_error::BuildDiagnostic;
use rustc_hash::FxHashSet;

use crate::types::DtsModule;

mod bind_imports_and_exports;
mod dts_linking_meta_data;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DtsLinkStage {
  pub modules: IndexVec<ModuleIdx, Option<DtsModule>>,
  pub entries: Vec<ModuleIdx>,
  pub symbols: SymbolRefDb,
  pub metas: DtsLinkingMetadataVec,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub used_symbol_refs: FxHashSet<SymbolRef>,
}

impl DtsLinkStage {
  pub fn new(
    modules: IndexVec<ModuleIdx, Option<DtsModule>>,
    entries: Vec<ModuleIdx>,
    symbols: SymbolRefDb,
  ) -> Self {
    Self {
      modules,
      entries,
      symbols,
      metas: IndexVec::default(),
      used_symbol_refs: HashSet::default(),
      warnings: vec![],
      errors: vec![],
    }
  }

  pub fn link(mut self) {
    self.bind_imports_and_exports();
  }
}
