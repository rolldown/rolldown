use rolldown_plugin_build_import_analysis::BuildImportAnalysisPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingBuildImportAnalysisPluginConfig {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}

impl TryFrom<BindingBuildImportAnalysisPluginConfig> for BuildImportAnalysisPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingBuildImportAnalysisPluginConfig) -> Result<Self, Self::Error> {
    Ok(Self {
      preload_code: value.preload_code,
      insert_preload: value.insert_preload,
      render_built_url: value.render_built_url,
      is_relative_base: value.is_relative_base,
    })
  }
}
