#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookLoadCallEnd {
  #[ts(type = "'HookLoadCallEnd'")]
  pub action: &'static str,
  pub module_id: String,
  pub source: Option<String>,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_index: u32,
}
