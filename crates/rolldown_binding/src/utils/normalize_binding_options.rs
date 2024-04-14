use std::{path::PathBuf, sync::Arc};

use crate::{
  options::plugin::JsPlugin,
  options::plugin::ParallelJsPlugin,
  types::{binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallbackExt},
  worker_manager::WorkerManager,
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
      let chunk = chunk.clone();
      Box::pin(async move {
        fn_js.await_call(RenderedChunk::from(chunk)).await.map_err(BuildError::from)
      })
    }))
  })
}

pub fn normalize_binding_options(
  input_options: crate::options::BindingInputOptions,
  output_options: crate::options::BindingOutputOptions,
  mut parallel_plugins_map: Option<crate::parallel_js_plugin_registry::PluginValues>,
  worker_manager: Option<WorkerManager>,
) -> napi::Result<NormalizeBindingOptionsReturn> {
  debug_assert!(PathBuf::from(&input_options.cwd) != PathBuf::from("/"), "{input_options:#?}");
  let cwd = PathBuf::from(input_options.cwd);

  let external = input_options.external.map(|ts_fn| {
    rolldown::External::Fn(Box::new(move |source, importer, is_resolved| {
      let ts_fn = ts_fn.clone();
      Box::pin(async move {
        ts_fn.call_async((source, importer, is_resolved)).await.map_err(BuildError::from)
      })
    }))
  });

  let bundler_options = BundlerOptions {
    input: Some(input_options.input.into_iter().map(Into::into).collect()),
    cwd: cwd.into(),
    external,
    treeshake: true.into(),
    resolve: input_options.resolve.map(Into::into),
    platform: input_options
      .platform
      .as_deref()
      .map(Platform::try_from)
      .transpose()
      .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
    shim_missing_exports: input_options.shim_missing_exports,
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
  let worker_manager = worker_manager.map(Arc::new);

  let plugins: Vec<BoxPlugin> = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .enumerate()
    .map(|(index, plugin)| {
      plugin.map_or_else(
        || {
          let plugins = parallel_plugins_map.as_mut().unwrap().remove(&index).unwrap();
          let worker_manager = worker_manager.as_ref().unwrap();
          ParallelJsPlugin::new_boxed(plugins, Arc::clone(worker_manager))
        },
        |plugin| JsPlugin::new_boxed(plugin),
      )
    })
    .collect::<Vec<_>>();

  Ok(NormalizeBindingOptionsReturn { bundler_options, plugins })
}
