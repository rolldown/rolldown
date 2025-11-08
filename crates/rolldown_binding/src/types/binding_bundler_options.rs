use crate::{
  options::{BindingInputOptions, BindingOutputOptions},
  parallel_js_plugin_registry::ParallelJsPluginRegistry,
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingBundlerOptions<'env> {
  pub input_options: BindingInputOptions<'env>,
  pub output_options: BindingOutputOptions<'env>,
  pub parallel_plugins_registry: Option<ParallelJsPluginRegistry>,
}
