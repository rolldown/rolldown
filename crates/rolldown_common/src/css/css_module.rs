use crate::side_effects::DeterminedSideEffects;
use crate::{ModuleIdx};
use arcstr::ArcStr;
use lightningcss::stylesheet::StyleSheet;

#[derive(Debug)]
pub struct CssModule {
  pub exec_order: u32,
  pub source: ArcStr,
  pub name: String,
  pub idx: ModuleIdx,
  pub side_effects: DeterminedSideEffects,
  pub ast: StyleSheet<'static, 'static>,
}
