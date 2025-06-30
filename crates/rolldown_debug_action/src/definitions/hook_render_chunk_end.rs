#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct HookRenderChunkEnd {
  #[ts(type = "'HookRenderChunkEnd'")]
  pub action: &'static str,
  pub plugin_name: String,
  /// The index of the plugin in the plugin list. It's unique to each plugin.
  pub plugin_index: u32,
  pub call_id: &'static str,
}
