#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookLoadCallEnd {
  #[ts(type = "'HookLoadCallEnd'")]
  pub kind: String,
  pub module_id: String,
  pub source: Option<String>,
  pub plugin_name: String,
}
