use crate::types::{InputItem, PluginItem};

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct SessionMeta {
  #[ts(type = "'SessionMeta'")]
  pub action: &'static str,
  pub inputs: Vec<InputItem>,
  pub plugins: Vec<PluginItem>,
  pub cwd: String,
  #[ts(type = "'browser' | 'node' | 'neutral'")]
  // Refer to crates/rolldown_common/src/inner_bundler_options/types/platform.rs
  pub platform: String,
  #[ts(type = "'esm' | 'cjs' | 'iife' | 'umd'")]
  // Refer to crates/rolldown_common/src/inner_bundler_options/types/output_format.rs
  pub format: String,
  /// `OutputOptions.dir`
  pub dir: Option<String>,
  /// `OutputOptions.file`
  pub file: Option<String>,
}
