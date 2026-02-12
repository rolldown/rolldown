use rolldown_plugin_bundle_analyzer::BundleAnalyzerPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingBundleAnalyzerPluginConfig {
  /// Output filename for the analysis data (default: "analyze-data.json")
  pub file_name: Option<String>,
}

impl From<BindingBundleAnalyzerPluginConfig> for BundleAnalyzerPlugin {
  fn from(value: BindingBundleAnalyzerPluginConfig) -> Self {
    Self { file_name: value.file_name }
  }
}
