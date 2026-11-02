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
  /// report (`metrics.json` + `module-graph.json` + markdown views + per-build `history.jsonl`) instead of the
  /// multi-GB `logs.json`, under `node_modules/.rolldown/report/<output dir>`. Any other value
  /// is an error.
  pub mode: Option<String>,
}
