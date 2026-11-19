use crate::options::{BindingInputOptions, BindingOutputOptions};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingBundlerOptions<'env> {
  pub input_options: BindingInputOptions<'env>,
  pub output_options: BindingOutputOptions<'env>,
}
