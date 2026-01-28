use rolldown_plugin_chunk_visualize::ChunkVisualizePlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingChunkVisualizePluginConfig {
  /// Output filename for the visualization data (default: "analyze-data.json" or "analyze-data.md")
  pub file_name: Option<String>,
  /// Output format: "json" (default) or "md" for LLM-friendly markdown
  #[napi(ts_type = "'json' | 'md'")]
  pub format: Option<String>,
}

impl From<BindingChunkVisualizePluginConfig> for ChunkVisualizePlugin {
  fn from(value: BindingChunkVisualizePluginConfig) -> Self {
    Self { file_name: value.file_name, format: value.format }
  }
}
