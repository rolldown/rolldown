#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingDevtoolsOptions {
  pub session_id: Option<String>,
  /// `"full"` (default): write JSON-lines devtools logs. `"metrics"`: aggregate the same
  /// event stream in-memory and emit an agent-readable report (`metrics.json` + markdown)
  /// instead.
  #[napi(ts_type = "'full' | 'metrics'")]
  pub mode: Option<String>,
  /// Metrics mode: output directory, relative to cwd (default: "node_modules/.rolldown/metrics").
  pub metrics_dir: Option<String>,
  /// Metrics mode: upper bound for every "top-N" list (default: 20).
  pub metrics_top_n: Option<u32>,
  /// Metrics mode: whether to emit a build-over-build delta (default: true).
  pub metrics_delta: Option<bool>,
}
