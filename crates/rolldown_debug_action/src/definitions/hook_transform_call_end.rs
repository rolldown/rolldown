#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookTransformCallEnd {
  #[ts(type = "'HookTransformCallEnd'")]
  pub kind: String,
  pub module_id: String,
  pub transformed_source: Option<String>,
  pub plugin_name: String,
}
