#[cfg_attr(target_family = "wasm", allow(unused))]
use crate::{
  options::plugin::JsPlugin,
  options::plugin::ParallelJsPlugin,
  types::{binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallbackExt},
  worker_manager::WorkerManager,
};
use napi::Either;
use rolldown::{AddonOutputOption, BundlerOptions, IsExternal, ModuleType, OutputFormat, Platform};
use rolldown_plugin::BoxPlugin;
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

#[cfg_attr(target_family = "wasm", allow(unused))]
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
        fn_js.await_call(RenderedChunk::from(chunk)).await.map_err(anyhow::Error::from)
      })
    }))
  })
}

#[allow(clippy::too_many_lines)]
pub fn normalize_binding_options(
  input_options: crate::options::BindingInputOptions,
  output_options: crate::options::BindingOutputOptions,
  #[cfg(not(target_family = "wasm"))] mut parallel_plugins_map: Option<
    crate::parallel_js_plugin_registry::PluginValues,
  >,
  #[cfg(not(target_family = "wasm"))] worker_manager: Option<WorkerManager>,
) -> napi::Result<NormalizeBindingOptionsReturn> {
  debug_assert!(PathBuf::from(&input_options.cwd) != PathBuf::from("/"), "{input_options:#?}");
  let cwd = PathBuf::from(input_options.cwd);

  let external = input_options.external.map(|ts_fn| {
    IsExternal::from_closure(move |source, importer, is_resolved| {
      let source = source.to_string();
      let importer = importer.map(ToString::to_string);
      let ts_fn = ts_fn.clone();
      Box::pin(async move {
        ts_fn
          .call_async((source.to_string(), importer.map(|v| v.to_string()), is_resolved))
          .await
          .map_err(anyhow::Error::from)
      })
    })
  });

  let sourcemap_ignore_list = output_options.sourcemap_ignore_list.map(|ts_fn| {
    rolldown::SourceMapIgnoreList::new(Box::new(move |source, sourcemap_path| {
      let ts_fn = ts_fn.clone();
      let source = source.to_string();
      let sourcemap_path = sourcemap_path.to_string();
      Box::pin(async move {
        ts_fn.call_async((source, sourcemap_path)).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let sourcemap_path_transform = output_options.sourcemap_path_transform.map(|ts_fn| {
    rolldown::SourceMapPathTransform::new(Box::new(move |source, sourcemap_path| {
      let ts_fn = ts_fn.clone();
      let source = source.to_string();
      let sourcemap_path = sourcemap_path.to_string();
      Box::pin(async move {
        ts_fn.call_async((source, sourcemap_path)).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let mut module_types = None;
  if let Some(raw) = input_options.module_types {
    let mut tmp = HashMap::with_capacity(raw.len());
    for (k, v) in raw {
      tmp.insert(
        k,
        ModuleType::from_str(&v)
          .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
      );
    }
    module_types = Some(tmp);
  }

  let bundler_options = BundlerOptions {
    input: Some(input_options.input.into_iter().map(Into::into).collect()),
    cwd: cwd.into(),
    external,
    treeshake: match input_options.treeshake {
      Some(v) => v.try_into().map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
      None => rolldown::TreeshakeOptions::False,
    },
    resolve: input_options.resolve.map(Into::into),
    platform: input_options
      .platform
      .as_deref()
      .map(Platform::try_from)
      .transpose()
      .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
    shim_missing_exports: input_options.shim_missing_exports,
    entry_filenames: output_options.entry_file_names,
    chunk_filenames: output_options.chunk_file_names,
    asset_filenames: output_options.asset_file_names,
    dir: output_options.dir,
    sourcemap: output_options.sourcemap.map(Into::into),
    banner: normalize_addon_option(output_options.banner),
    footer: normalize_addon_option(output_options.footer),
    sourcemap_ignore_list,
    sourcemap_path_transform,
    format: output_options.format.map(|format_str| match format_str.as_str() {
      "es" => OutputFormat::Esm,
      "cjs" => OutputFormat::Cjs,
      _ => panic!("Invalid format: {format_str}"),
    }),
    module_types,
  };

  #[cfg(not(target_family = "wasm"))]
  // Deal with plugins
  let worker_manager = worker_manager.map(Arc::new);

  #[cfg(not(target_family = "wasm"))]
  let plugins: Vec<BoxPlugin> = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .enumerate()
    .map(|(index, plugin)| {
      plugin.map_or_else(
        || {
          let plugins = parallel_plugins_map
            .as_mut()
            .and_then(|plugin| plugin.remove(&index))
            .unwrap_or_default();
          let worker_manager = worker_manager.as_ref().unwrap();
          ParallelJsPlugin::new_boxed(plugins, Arc::clone(worker_manager))
        },
        |plugin| match plugin {
          Either::A(plugin) => JsPlugin::new_boxed(plugin),
          Either::B(plugin) => plugin.into(),
        },
      )
    })
    .collect::<Vec<_>>();

  #[cfg(target_family = "wasm")]
  let plugins: Vec<BoxPlugin> = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .filter_map(|plugin| {
      plugin.map(|plugin| match plugin {
        Either::A(plugin) => JsPlugin::new_boxed(plugin),
        Either::B(plugin) => plugin.into(),
      })
    })
    .collect::<Vec<_>>();

  Ok(NormalizeBindingOptionsReturn { bundler_options, plugins })
}
