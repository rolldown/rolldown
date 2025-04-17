#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookLoadCallStart {
  #[ts(type = "'HookLoadCallStart'")]
  pub kind: String,
  pub module_id: String,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_index: u32,
}
