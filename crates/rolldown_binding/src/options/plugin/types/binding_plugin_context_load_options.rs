/// The options of `BindingPluginContext.load` that a call can leave out.
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingPluginContextLoadOptions {
  pub package_json_path: Option<String>,
  /// The receipt of the module options that `this.load` saved on the JS side. They apply only
  /// when this call creates the module.
  pub module_options_receipt: Option<u32>,
}
