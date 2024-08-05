use arcstr::ArcStr;

use super::plugin_idx::PluginIdx;

#[derive(Debug, Clone)]
pub struct HookResolveIdSkipped {
  pub importer: Option<ArcStr>,
  pub plugin_idx: PluginIdx,
  pub specifier: ArcStr,
}
