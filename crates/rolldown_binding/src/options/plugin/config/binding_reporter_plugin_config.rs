use rolldown_plugin_reporter::ReporterPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingReporterPluginConfig {
  pub is_tty: bool,
  pub is_lib: bool,
  pub assets_dir: String,
  pub chunk_limit: u32,
  pub should_log_info: bool,
  pub report_compressed_size: bool,
}

impl From<BindingReporterPluginConfig> for ReporterPlugin {
  fn from(config: BindingReporterPluginConfig) -> Self {
    ReporterPlugin::new(
      config.is_tty,
      config.should_log_info,
      config.chunk_limit as usize,
      config.report_compressed_size,
      config.assets_dir,
      config.is_lib,
    )
  }
}
