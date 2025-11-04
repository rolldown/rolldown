use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_build_import_analysis::{
  BuildImportAnalysisPlugin, BuildImportAnalysisPluginV2,
};
use rolldown_plugin_vite_html::ResolveDependenciesFn;

use crate::{
  options::plugin::{
    config::binding_vite_html_plugin_config::BindingResolveDependenciesContext,
    types::binding_render_built_url::BindingRenderBuiltUrl,
  },
  types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingBuildImportAnalysisPluginV2Config {
  pub is_ssr: bool,
  pub url_base: String,
  pub decoded_base: String,
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => Promise<undefined | string | BindingRenderBuiltUrlRet>"
  )]
  pub render_built_url: Option<BindingRenderBuiltUrl>,
  #[napi(
    ts_type = "boolean | ((filename: string, dependencies: string[], context: { hostId: string, hostType: 'html' | 'js' }) => Promise<string[]>)"
  )]
  pub resolve_dependencies: Option<
    MaybeAsyncJsCallback<
      FnArgs<(String, Vec<String>, BindingResolveDependenciesContext)>,
      Vec<String>,
    >,
  >,
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
      v2: value.v2.map(|v2_config| {
        let resolve_dependencies = v2_config.resolve_dependencies.map(
          |resolve_dependencies| -> Arc<ResolveDependenciesFn> {
            Arc::new(move |filename: &str, deps: Vec<String>, host_id: &str, host_type: &str| {
              let filename = filename.to_string();
              let context = BindingResolveDependenciesContext {
                host_id: host_id.to_string(),
                host_type: host_type.to_string(),
              };

              let resolve_dependencies = Arc::clone(&resolve_dependencies);
              Box::pin(async move {
                resolve_dependencies
                  .await_call((filename, deps, context).into())
                  .await
                  .map_err(anyhow::Error::from)
              })
            })
          },
        );
        BuildImportAnalysisPluginV2 {
          resolve_dependencies,
          is_ssr: v2_config.is_ssr,
          url_base: v2_config.url_base,
          decoded_base: v2_config.decoded_base,
          render_built_url: v2_config.render_built_url.map(Into::into),
        }
      }),
    })
  }
}
