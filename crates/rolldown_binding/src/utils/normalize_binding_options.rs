use std::path::PathBuf;

use rolldown::{InputOptions, OutputOptions};
use rolldown_plugin::BoxPlugin;

use crate::{
  options::{plugin::JsAdapterPlugin, ExternalFn},
  utils::napi_error_ext::NapiErrorExt,
};

pub struct NormalizeBindingOptionsReturn {
  pub input_options: InputOptions,
  pub output_options: OutputOptions,
  pub plugins: Vec<BoxPlugin>,
}

pub fn normalize_binding_options(
  input_options: crate::options::BindingInputOptions,
  output_options: crate::options::BindingOutputOptions,
) -> napi::Result<NormalizeBindingOptionsReturn> {
  // Deal with input options

  debug_assert!(PathBuf::from(&input_options.cwd) != PathBuf::from("/"), "{input_options:#?}");
  let cwd = PathBuf::from(input_options.cwd);

  let external = input_options
    .external
    .map(|js_fn| {
      ExternalFn::new(&js_fn).map(|external_fn| {
        let cb = Box::new(external_fn);
        rolldown::External::Fn(Box::new(move |source, importer, is_resolved| {
          let ts_fn = Box::clone(&cb);
          Box::pin(async move {
            ts_fn
              .call_async((source, importer, is_resolved))
              .await
              .map_err(NapiErrorExt::into_bundle_error)
          })
        }))
      })
    })
    .transpose()?
    .unwrap_or_default();

  let normalized_input_options = InputOptions {
    input: input_options.input.into_iter().map(Into::into).collect(),
    cwd: cwd.into(),
    external: external.into(),
    treeshake: true.into(),
    resolve: input_options.resolve.map(Into::into),
  };

  // Deal with output options

  let normalized_output_options = OutputOptions {
    entry_file_names: output_options.entry_file_names,
    chunk_file_names: output_options.chunk_file_names,
    dir: output_options.dir,
    sourcemap: output_options.sourcemap.map(Into::into),
    ..Default::default()
  };

  // Deal with plugins

  let plugins = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .map(JsAdapterPlugin::new_boxed)
    .collect::<napi::Result<Vec<_>>>()?;

  Ok(NormalizeBindingOptionsReturn {
    input_options: normalized_input_options,
    output_options: normalized_output_options,
    plugins,
  })
}
