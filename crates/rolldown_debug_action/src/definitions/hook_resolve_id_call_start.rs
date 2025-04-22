#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookResolveIdCallStart {
  #[ts(type = "'HookResolveIdCallStart'")]
  pub kind: &'static str,
  pub importer: Option<String>,
  pub module_request: String,
  pub import_kind: String,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_index: u32,
  // #[serde(serialize_with = "crate::serialize_with_provided_data")]
  pub trigger: &'static str,
}
