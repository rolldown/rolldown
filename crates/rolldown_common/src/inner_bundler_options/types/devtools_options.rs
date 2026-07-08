#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DevtoolsOptions {
  pub session_id: Option<String>,
  /// `"full"` (default): write JSON-lines devtools logs to disk.
  /// `"metrics"`: aggregate the same event stream in-memory and emit an agent-readable
  /// report (`metrics.json` + markdown views + per-build `history.jsonl`) instead of the
  /// multi-GB `logs.json`.
  pub mode: Option<String>,
  /// Metrics mode: directory the report is written to, relative to cwd
  /// (default: `node_modules/.rolldown/metrics`).
  pub metrics_dir: Option<String>,
  /// Metrics mode: upper bound for every "top-N" list, so output stays small regardless of
  /// app size (default: 20).
  pub metrics_top_n: Option<u32>,
  /// Metrics mode: whether to emit a build-over-build delta (default: true).
  pub metrics_delta: Option<bool>,
}
