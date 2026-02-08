use rolldown_plugin_chunk_visualize::ChunkVisualizePlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingChunkVisualizePluginConfig {
  /// Output filename for the visualization data (default: "analyze-data.json")
  pub file_name: Option<String>,
}

impl From<BindingChunkVisualizePluginConfig> for ChunkVisualizePlugin {
  fn from(value: BindingChunkVisualizePluginConfig) -> Self {
    Self { file_name: value.file_name }
  }
}
