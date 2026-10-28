#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingDevtoolsOptions {
  pub session_id: Option<String>,
  /// `"full"` (default): write JSON-lines devtools logs. `"report"`: aggregate the same
  /// event stream in-memory and emit an agent-readable report (`metrics.json` + markdown)
  /// instead.
  #[napi(ts_type = "'full' | 'report'")]
  pub mode: Option<String>,
  /// Report mode: output directory, relative to cwd (default: "node_modules/.rolldown/report").
  pub report_dir: Option<String>,
  /// Report mode: upper bound for every "top-N" list (default: 20).
  pub report_top_n: Option<u32>,
  /// Report mode: whether to emit a build-over-build delta (default: true).
  pub report_delta: Option<bool>,
}
