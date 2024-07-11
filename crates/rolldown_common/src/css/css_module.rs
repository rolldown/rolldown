use crate::side_effects::DeterminedSideEffects;
use crate::ModuleIdx;
use std::sync::Arc;

#[derive(Debug)]
pub struct CssModule {
  pub exec_order: u32,
  pub source: Arc<str>,
  pub name: String,
  pub idx: ModuleIdx,
  pub side_effects: DeterminedSideEffects,
  pub ast: lightningcss::stylesheet::StyleSheet<'static, 'static>,
}
