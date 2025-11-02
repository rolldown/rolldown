use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use rolldown_plugin_build_import_analysis::BuildImportAnalysisPlugin;
use rolldown_plugin_vite_html::ResolveDependenciesFn;

use crate::{
  options::plugin::config::binding_vite_html_plugin_config::BindingResolveDependenciesContext,
  types::js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
};

#[napi_derive::napi(object, object_to_js = false)]
#[expect(clippy::struct_excessive_bools)]
pub struct BindingBuildImportAnalysisPluginConfig {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub is_test_v2: Option<bool>,
  pub is_module_preload: Option<bool>,
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

impl TryFrom<BindingBuildImportAnalysisPluginConfig> for BuildImportAnalysisPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingBuildImportAnalysisPluginConfig) -> Result<Self, Self::Error> {
    let resolve_dependencies =
      value.resolve_dependencies.map(|resolve_dependencies| -> Arc<ResolveDependenciesFn> {
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
      });

    Ok(Self {
      preload_code: value.preload_code.into(),
      insert_preload: value.insert_preload,
      render_built_url: value.render_built_url,
      is_relative_base: value.is_relative_base,
      is_test_v2: value.is_test_v2.unwrap_or_default(),
      is_module_preload: value.is_module_preload.unwrap_or_default(),
      resolve_dependencies,
    })
  }
}
