#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookTransformCallEnd {
  #[ts(type = "'HookTransformCallEnd'")]
  pub action: &'static str,
  pub module_id: String,
  /// Result after transform. Empty means the transform hook returns nothing.
  pub content: Option<String>,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_id: u32,
  pub call_id: String,
}
