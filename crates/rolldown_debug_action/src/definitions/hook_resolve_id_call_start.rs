#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookResolveIdCallStart {
  #[ts(type = "'HookResolveIdCallStart'")]
  pub action: &'static str,
  pub importer: Option<String>,
  pub module_request: String,
  pub import_kind: String,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_id: u32,
  #[ts(type = "'automatic' | 'manual'")]
  /// - `automatic` means the resolve call is triggered by the bundler automatically.
  /// - `manual` means the resolve call is triggered manually by `this.resolve(...)`
  pub trigger: &'static str,
  pub call_id: &'static str,
}
