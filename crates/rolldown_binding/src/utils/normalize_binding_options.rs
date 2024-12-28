use crate::options::ChunkFileNamesOutputOption;
use crate::{
  options::binding_inject_import::normalize_binding_inject_import,
  types::js_callback::JsCallbackExt,
};
#[cfg_attr(target_family = "wasm", allow(unused))]
use crate::{
  options::plugin::JsPlugin,
  types::{binding_rendered_chunk::RenderedChunk, js_callback::MaybeAsyncJsCallbackExt},
};
use napi::bindgen_prelude::Either;
use rolldown::{
  AddonOutputOption, AdvancedChunksOptions, BundlerOptions, ChunkFilenamesOutputOption,
  ExperimentalOptions, HashCharacters, IsExternal, MatchGroup, ModuleType, OutputExports,
  OutputFormat, Platform,
};
use rolldown_plugin::__inner::SharedPluginable;
use rolldown_utils::indexmap::FxIndexMap;
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::FxHashMap;
use std::path::PathBuf;

#[cfg(not(target_family = "wasm"))]
use crate::{options::plugin::ParallelJsPlugin, worker_manager::WorkerManager};
use std::sync::Arc;

#[cfg_attr(target_family = "wasm", allow(unused))]
pub struct NormalizeBindingOptionsReturn {
  pub bundler_options: BundlerOptions,
  pub plugins: Vec<SharedPluginable>,
}

fn normalize_addon_option(
  addon_option: Option<crate::options::AddonOutputOption>,
) -> Option<AddonOutputOption> {
  addon_option.map(move |value| {
    AddonOutputOption::Fn(Arc::new(move |chunk| {
      let fn_js = Arc::clone(&value);
      let chunk = chunk.clone();
      Box::pin(async move {
        fn_js.await_call(RenderedChunk::from(chunk)).await.map_err(anyhow::Error::from)
      })
    }))
  })
}

fn normalize_chunk_file_names_option(
  option: Option<ChunkFileNamesOutputOption>,
) -> napi::Result<Option<ChunkFilenamesOutputOption>> {
  option
    .map(move |value| match value {
      Either::A(str) => Ok(ChunkFilenamesOutputOption::String(str)),
      Either::B(func) => Ok(ChunkFilenamesOutputOption::Fn(Arc::new(move |chunk| {
        let func = Arc::clone(&func);
        let chunk = chunk.clone();
        Box::pin(async move { func.invoke_async(chunk.into()).await.map_err(anyhow::Error::from) })
      }))),
    })
    .transpose()
}

fn normalize_globals_option(
  option: Option<crate::options::GlobalsOutputOption>,
) -> Option<rolldown_common::GlobalsOutputOption> {
  option.map(move |value| match value {
    Either::A(hash_map) => {
      rolldown_common::GlobalsOutputOption::FxHashMap(hash_map.into_iter().collect())
    }
    Either::B(func) => rolldown_common::GlobalsOutputOption::Fn(Arc::new(move |name| {
      let func = Arc::clone(&func);
      let name = name.to_string();
      Box::pin(async move { func.invoke_async(name).await.map_err(anyhow::Error::from) })
    })),
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
  let cwd = PathBuf::from(input_options.cwd);

  let external = input_options.external.map(|ts_fn| {
    IsExternal::from_closure(move |source, importer, is_resolved| {
      let source = source.to_string();
      let importer = importer.map(ToString::to_string);
      let ts_fn = Arc::clone(&ts_fn);
      Box::pin(async move {
        ts_fn
          .invoke_async((source.to_string(), importer.map(|v| v.to_string()), is_resolved))
          .await
          .map_err(anyhow::Error::from)
      })
    })
  });

  let sourcemap_ignore_list = output_options.sourcemap_ignore_list.map(|ts_fn| {
    rolldown::SourceMapIgnoreList::new(Arc::new(move |source, sourcemap_path| {
      let ts_fn = Arc::clone(&ts_fn);
      let source = source.to_string();
      let sourcemap_path = sourcemap_path.to_string();
      Box::pin(async move {
        ts_fn.invoke_async((source, sourcemap_path)).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let sourcemap_path_transform = output_options.sourcemap_path_transform.map(|ts_fn| {
    rolldown::SourceMapPathTransform::new(Arc::new(move |source, sourcemap_path| {
      let ts_fn = Arc::clone(&ts_fn);
      let source = source.to_string();
      let sourcemap_path = sourcemap_path.to_string();
      Box::pin(async move {
        ts_fn.invoke_async((source, sourcemap_path)).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let mut module_types = None;
  if let Some(raw) = input_options.module_types {
    let mut tmp: FxHashMap<_, _> = FxHashMapExt::with_capacity(raw.len());
    for (k, v) in raw {
      tmp.insert(
        k,
        ModuleType::from_known_str(&v)
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
      None => rolldown::TreeshakeOptions::Boolean(false),
    },
    resolve: input_options.resolve.map(Into::into),
    platform: input_options
      .platform
      .as_deref()
      .map(Platform::try_from)
      .transpose()
      .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?,
    shim_missing_exports: input_options.shim_missing_exports,
    name: output_options.name,
    asset_filenames: output_options.asset_file_names,
    entry_filenames: normalize_chunk_file_names_option(output_options.entry_file_names)?,
    chunk_filenames: normalize_chunk_file_names_option(output_options.chunk_file_names)?,
    css_entry_filenames: normalize_chunk_file_names_option(output_options.css_entry_file_names)?,
    css_chunk_filenames: normalize_chunk_file_names_option(output_options.css_chunk_file_names)?,
    dir: output_options.dir,
    file: output_options.file,
    sourcemap: output_options.sourcemap.map(Into::into),
    es_module: output_options.es_module.map(|es_module| match es_module {
      Either::A(es_module_bool) => es_module_bool.into(),
      Either::B(es_module_string) => es_module_string.into(),
    }),
    banner: normalize_addon_option(output_options.banner),
    footer: normalize_addon_option(output_options.footer),
    intro: normalize_addon_option(output_options.intro),
    outro: normalize_addon_option(output_options.outro),
    sourcemap_ignore_list,
    sourcemap_path_transform,
    sourcemap_debug_ids: output_options.sourcemap_debug_ids,
    exports: output_options.exports.map(|format_str| match format_str.as_str() {
      "auto" => OutputExports::Auto,
      "default" => OutputExports::Default,
      "named" => OutputExports::Named,
      "none" => OutputExports::None,
      _ => panic!("Invalid exports: {format_str}"),
    }),
    format: output_options.format.map(|format_str| match format_str.as_str() {
      "es" => OutputFormat::Esm,
      "cjs" => OutputFormat::Cjs,
      "app" => OutputFormat::App,
      "iife" => OutputFormat::Iife,
      "umd" => OutputFormat::Umd,
      _ => panic!("Invalid format: {format_str}"),
    }),
    hash_characters: output_options.hash_characters.map(|format_str| match format_str.as_str() {
      "base64" => HashCharacters::Base64,
      "base36" => HashCharacters::Base36,
      "hex" => HashCharacters::Hex,
      _ => panic!("Invalid hash characters: {format_str}"),
    }),
    globals: normalize_globals_option(output_options.globals),
    module_types,
    experimental: input_options.experimental.map(|inner| ExperimentalOptions {
      strict_execution_order: inner.strict_execution_order,
      disable_live_bindings: inner.disable_live_bindings,
      vite_mode: inner.vite_mode,
      resolve_new_url_to_asset: inner.resolve_new_url_to_asset,
    }),
    minify: output_options.minify,
    extend: output_options.extend,
    define: input_options.define.map(FxIndexMap::from_iter),
    inject: input_options
      .inject
      .map(|inner| inner.into_iter().map(normalize_binding_inject_import).collect()),
    external_live_bindings: output_options.external_live_bindings,
    inline_dynamic_imports: output_options.inline_dynamic_imports,
    advanced_chunks: output_options.advanced_chunks.map(|inner| AdvancedChunksOptions {
      min_size: inner.min_size,
      min_share_count: inner.min_share_count,
      groups: inner.groups.map(|inner| {
        inner
          .into_iter()
          .map(|item| MatchGroup {
            name: item.name,
            test: item.test.map(|inner| inner.try_into().expect("Invalid regex pass to test")),
            priority: item.priority,
            min_size: item.min_size,
            min_share_count: item.min_share_count,
          })
          .collect::<Vec<_>>()
      }),
    }),
    checks: input_options.checks.map(Into::into),
    profiler_names: input_options.profiler_names,
    jsx: input_options.jsx.map(Into::into),
    watch: input_options.watch.map(TryInto::try_into).transpose()?,
    comments: output_options
      .comments
      .map(|inner| match inner.as_str() {
        "none" => Ok(rolldown::Comments::None),
        "preserve-legal" => Ok(rolldown::Comments::Preserve),
        _ => Err(napi::Error::new(
          napi::Status::GenericFailure,
          format!("Invalid valid for `comments` option: {inner}"),
        )),
      })
      .transpose()?,
    drop_labels: input_options.drop_labels,
    target: output_options.target.as_deref().map(std::str::FromStr::from_str).transpose()?,
    keep_names: input_options.keep_names,
    polyfill_require: output_options.polyfill_require,
  };

  #[cfg(not(target_family = "wasm"))]
  // Deal with plugins
  let worker_manager = worker_manager.map(Arc::new);

  #[cfg(not(target_family = "wasm"))]
  let plugins: Vec<SharedPluginable> = input_options
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
          ParallelJsPlugin::new_shared(plugins, Arc::clone(worker_manager))
        },
        |plugin| match plugin {
          Either::A(plugin_options) => JsPlugin::new_shared(plugin_options),
          Either::B(builtin) => {
            // Needs to save the name, since `try_into` will consume the ownership
            let name = format!("{:?}", builtin.__name);
            builtin
              .try_into()
              .unwrap_or_else(|err| panic!("Should convert to builtin plugin: {name} \n {err}"))
          }
        },
      )
    })
    .collect::<Vec<_>>();

  #[cfg(target_family = "wasm")]
  let plugins: Vec<SharedPluginable> = input_options
    .plugins
    .into_iter()
    .chain(output_options.plugins)
    .filter_map(|plugin| {
      plugin.map(|plugin| match plugin {
        Either::A(plugin_options) => JsPlugin::new_shared(plugin_options),
        Either::B(builtin) => {
          // Needs to save the name, since `try_into` will consume the ownership
          let name = format!("{:?}", builtin.__name);
          builtin
            .try_into()
            .unwrap_or_else(|err| panic!("Should convert to builtin plugin: {name} \n {err}"))
        }
      })
    })
    .collect::<Vec<_>>();

  Ok(NormalizeBindingOptionsReturn { bundler_options, plugins })
}
