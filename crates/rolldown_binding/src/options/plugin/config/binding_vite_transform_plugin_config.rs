use std::path::PathBuf;

use oxc_transform_napi::TransformOptions;
use rolldown_plugin_vite_transform::ViteTransformPlugin;
use sugar_path::SugarPath;

use crate::{
  types::binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  utils::normalize_binding_transform_options,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingViteTransformPluginConfig {
  pub root: String,

  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_include: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_exclude: Option<Vec<BindingStringOrRegex>>,

  pub is_server_consumer: Option<bool>,

  pub jsx_inject: Option<String>,
  pub transform_options: Option<TransformOptions>,

  pub yarn_pnp: Option<bool>,
}

impl From<BindingViteTransformPluginConfig> for ViteTransformPlugin {
  fn from(value: BindingViteTransformPluginConfig) -> Self {
    Self {
      root: PathBuf::from(value.root).normalize(),
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      jsx_refresh_include: value
        .jsx_refresh_include
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      jsx_refresh_exclude: value
        .jsx_refresh_exclude
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      jsx_inject: value.jsx_inject,
      is_server_consumer: value.is_server_consumer.unwrap_or(true),
      sourcemap: value.transform_options.as_ref().and_then(|v| v.sourcemap).unwrap_or(true),
      transform_options: value
        .transform_options
        .map(normalize_binding_transform_options)
        .unwrap_or_default(),
      resolver: ViteTransformPlugin::new_resolver(value.yarn_pnp.unwrap_or_default()),
    }
  }
}
