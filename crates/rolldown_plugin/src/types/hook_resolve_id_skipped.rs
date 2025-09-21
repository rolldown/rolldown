use arcstr::ArcStr;
use rolldown_common::PluginIdx;

#[derive(Debug, Clone)]
pub struct HookResolveIdSkipped {
  pub importer: Option<ArcStr>,
  pub plugin_idx: PluginIdx,
  pub specifier: ArcStr,
}
