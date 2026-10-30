#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingDevtoolsOptions {
  pub session_id: Option<String>,
  /// `"full"` (default): write JSON-lines devtools logs. `"report"`: aggregate the same
  /// event stream in-memory and emit an agent-readable report (`metrics.json` + markdown)
  /// under `node_modules/.rolldown/report/<output dir>` instead.
  #[napi(ts_type = "'full' | 'report'")]
  pub mode: Option<String>,
}
