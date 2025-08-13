use derive_more::Debug;
use oxc_minify_napi::MinifyOptions;
#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingMinifyOptions {
  #[debug(skip)]
  pub minify_options: MinifyOptions,
}

impl From<BindingMinifyOptions> for rolldown_common::RawMinifyOptions {
  fn from(value: BindingMinifyOptions) -> Self {
    Self::Object(rolldown_common::MinifyOptionsObject {
      module: value.minify_options.module,
      mangle: match value.minify_options.mangle {
        None | Some(napi::Either::A(false)) => false,
        Some(_) => true,
      },
      compress: match value.minify_options.compress {
        None | Some(napi::Either::A(false)) => false,
        Some(_) => true,
      },
      remove_whitespace: match value.minify_options.codegen {
        None => false,
        Some(napi::Either::A(false)) => false,
        Some(_) => true,
      },
      sourcemap: value.minify_options.sourcemap.unwrap_or_default(),
    })
  }
}

impl From<&rolldown_common::MinifyOptionsObject> for BindingMinifyOptions {
  fn from(value: &rolldown_common::MinifyOptionsObject) -> Self {
    Self {
      minify_options: MinifyOptions {
        module: value.module,
        mangle: match value.mangle {
          false => Some(napi::Either::A(false)),
          true => Some(napi::Either::A(true)),
        },
        compress: match value.compress {
          false => Some(napi::Either::A(false)),
          true => Some(napi::Either::A(true)),
        },
        codegen: Some(napi::Either::A(value.remove_whitespace)),

        sourcemap: Some(value.sourcemap),
      },
    }
  }
}
