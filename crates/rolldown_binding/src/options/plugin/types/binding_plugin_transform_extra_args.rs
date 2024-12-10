#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingTransformHookExtraArgs {
  pub module_type: String,
}
