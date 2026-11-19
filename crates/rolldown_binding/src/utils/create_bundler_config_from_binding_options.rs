use crate::{
  types::binding_bundler_options::BindingBundlerOptions,
  utils::normalize_binding_options::{NormalizedBindingOptions, normalize_binding_options},
};

/// The caller owns the `callbacks` field of the result. The caller must release these JS functions
/// at the end of its close.
pub fn create_bundler_config_from_binding_options(
  option: BindingBundlerOptions,
) -> napi::Result<NormalizedBindingOptions> {
  let BindingBundlerOptions { input_options, output_options } = option;
  normalize_binding_options(input_options, output_options)
}
