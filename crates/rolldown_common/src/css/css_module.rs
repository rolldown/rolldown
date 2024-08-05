use arcstr::ArcStr;

use super::css_module_idx::CssModuleIdx;

pub struct CssModule {
  pub exec_order: u32,
  pub source: ArcStr,
  pub idx: CssModuleIdx,
  pub ast: lightningcss::stylesheet::StyleSheet<'static, 'static>,
}
