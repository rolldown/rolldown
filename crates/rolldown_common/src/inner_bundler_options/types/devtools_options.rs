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
  /// `"report"`: aggregate the same event stream in-memory and emit an agent-readable
  /// report (`metrics.json` + markdown views + per-build `history.jsonl`) instead of the
  /// multi-GB `logs.json`. Any other value is an error.
  pub mode: Option<String>,
  /// Report mode: directory the report is written to, relative to cwd
  /// (default: `node_modules/.rolldown/report`).
  pub report_dir: Option<String>,
  /// Report mode: upper bound for every "top-N" list, so output stays small regardless of
  /// app size (default: 20).
  pub report_top_n: Option<u32>,
  /// Report mode: whether to emit a build-over-build delta (default: true).
  pub report_delta: Option<bool>,
}
