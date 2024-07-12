use crate::side_effects::DeterminedSideEffects;
use crate::{ImportRecord, ImportRecordIdx, ModuleIdx};
use arcstr::ArcStr;
use lightningcss::stylesheet::StyleSheet;
use oxc::index::IndexVec;

#[derive(Debug)]
pub struct CssModule {
  pub exec_order: u32,
  pub source: ArcStr,
  pub name: String,
  pub idx: ModuleIdx,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
  pub side_effects: DeterminedSideEffects,
  pub ast: StyleSheet<'static, 'static>,
}

impl CssModule {
  pub fn new(
    idx: ModuleIdx,
    source: ArcStr,
    name: String,
    side_effects: DeterminedSideEffects,
  ) -> Self {
    let ast = rolldown_css::css_ast::parse_to_css_ast(&source);
    Self {
      exec_order: u32::MAX,
      source,
      name,
      idx,
      import_records: IndexVec::default(),
      side_effects,
      ast,
    }
  }
}
