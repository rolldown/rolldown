use arcstr::ArcStr;
use oxc::index::IndexVec;

use crate::{
  side_effects::DeterminedSideEffects, types::css_ast_idx::CssAstIdx, ImportRecord,
  ImportRecordIdx, ModuleId, ModuleIdx,
};

#[derive(Debug)]
pub struct CssModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub id: ModuleId,
  pub stable_id: String,
  pub ast_idx: Option<CssAstIdx>,
  pub source: ArcStr,
  pub side_effects: DeterminedSideEffects,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
}

impl CssModule {
  pub fn css_ast_idx(&self) -> CssAstIdx {
    self.ast_idx.expect("CssAstIdx should be set")
  }
}
