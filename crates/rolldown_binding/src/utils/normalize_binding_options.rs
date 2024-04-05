use std::path::PathBuf;

use crate::{
  options::plugin::JsPlugin,
  types::{binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallbackExt},
};
use rolldown::{AddonOutputOption, BundlerOptions, Platform};
use rolldown_error::BuildError;
use rolldown_plugin::BoxPlugin;

pub struct NormalizeBindingOptionsReturn {
  pub bundler_options: BundlerOptions,
  pub plugins: Vec<BoxPlugin>,
}

fn normalize_addon_option(
  addon_option: Option<crate::options::AddonOutputOption>,
) -> Option<AddonOutputOption> {
  addon_option.map(move |value| {
    AddonOutputOption::Fn(Box::new(move |chunk| {
      let fn_js = value.clone();
      Box::pin(async move {
        fn_js.await_call(RenderedChunk::from(chunk)).await.map_err(BuildError::from)
      })
    }))
  })
}

pub fn normalize_binding_options(
  input_options: crate::options::BindingInputOptions,
  output_options: crate::options::BindingOutputOptions,
) -> napi::Result<NormalizeBindingOptionsReturn> {
  debug_assert!(PathBuf::from(&input_options.cwd) != PathBuf::from("/"), "{input_options:#?}");
  let cwd = PathBuf::from(input_options.cwd);

  let external = input_options
    .external
    .map(|ts_fn| {
      rolldown::External::Fn(Box::new(move |source, importer, is_resolved| {
        let ts_fn = ts_fn.clone();
        Box::pin(async move {
          ts_fn.call_async((source, importer, is_resolved)).await.map_err(BuildError::from)
        })
      }))
    })
    .unwrap_or_default();

  let bundler_options = BundlerOptions {
    input: input_options.input.into_iter().map(Into::into).collect(),
    cwd: cwd.into(),
    external: external.into(),
    treeshake: true.into(),
    resolve: input_options.resolve.map(Into::into),
    platform: input_options
      .platform
      .as_deref()
      .map(Platform::try_from)
      .transpose()
      .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
    entry_file_names: output_options.entry_file_names,
    chunk_file_names: output_options.chunk_file_names,
    dir: output_options.dir,
    sourcemap: output_options.sourcemap.map(Into::into),
    banner: normalize_addon_option(output_options.banner),
    footer: normalize_addon_option(output_options.footer),
    // TODO(hyf0): remove this line, all options should set explicitly
    ..Default::default()
  };

  // Deal with plugins

  let plugins = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .map(JsPlugin::new_boxed)
    .collect::<Vec<_>>();

  Ok(NormalizeBindingOptionsReturn { bundler_options, plugins })
}
