use rolldown_plugin_build_import_analysis::{
  BuildImportAnalysisPlugin, BuildImportAnalysisPluginV2,
};

use crate::options::plugin::types::{
  binding_module_preload::BindingModulePreload, binding_render_built_url::BindingRenderBuiltUrl,
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingBuildImportAnalysisPluginV2Config {
  pub is_ssr: bool,
  pub url_base: String,
  pub decoded_base: String,
  #[napi(ts_type = "false | BindingModulePreloadOptions")]
  pub module_preload: BindingModulePreload,
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => undefined | string | BindingRenderBuiltUrlRet"
  )]
  pub render_built_url: Option<BindingRenderBuiltUrl>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[expect(clippy::struct_excessive_bools)]
pub struct BindingBuildImportAnalysisPluginConfig {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub v2: Option<BindingBuildImportAnalysisPluginV2Config>,
}

impl TryFrom<BindingBuildImportAnalysisPluginConfig> for BuildImportAnalysisPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingBuildImportAnalysisPluginConfig) -> Result<Self, Self::Error> {
    Ok(Self {
      preload_code: value.preload_code.into(),
      insert_preload: value.insert_preload,
      render_built_url: value.render_built_url,
      is_relative_base: value.is_relative_base,
      v2: value.v2.map(|v2_config| BuildImportAnalysisPluginV2 {
        is_ssr: v2_config.is_ssr,
        url_base: v2_config.url_base,
        decoded_base: v2_config.decoded_base,
        module_preload: v2_config.module_preload.into(),
        render_built_url: v2_config.render_built_url.map(Into::into),
      }),
    })
  }
}
