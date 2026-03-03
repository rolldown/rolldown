use rolldown_plugin_bundle_analyzer::BundleAnalyzerPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingBundleAnalyzerPluginConfig {
  /// Output filename for the bundle analysis data (default: "analyze-data.json")
  pub file_name: Option<String>,
  /// Output format: "json" (default) or "md" for LLM-friendly markdown
  #[napi(ts_type = "'json' | 'md'")]
  pub format: Option<String>,
}

impl From<BindingBundleAnalyzerPluginConfig> for BundleAnalyzerPlugin {
  fn from(value: BindingBundleAnalyzerPluginConfig) -> Self {
    Self { file_name: value.file_name, format: value.format }
  }
}
