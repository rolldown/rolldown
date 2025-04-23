#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookResolveIdCallEnd {
  #[ts(type = "'HookResolveIdCallEnd'")]
  pub action: &'static str,
  pub resolved_id: Option<String>,
  /// If the value is `true/false`, it means the plugin explicitly returned the value for this field.
  pub is_external: Option<bool>,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_index: u32,
}
