use crate::options::binding_advanced_chunks_options::BindingChunkingContext;
use crate::options::binding_jsx::BindingJsx;
use crate::options::{AssetFileNamesOutputOption, ChunkFileNamesOutputOption, SanitizeFileName};
use crate::{
  options::binding_inject_import::normalize_binding_inject_import,
  types::js_callback::JsCallbackExt,
};
#[cfg_attr(target_family = "wasm", allow(unused))]
use crate::{
  options::plugin::JsPlugin,
  types::{binding_rendered_chunk::BindingRenderedChunk, js_callback::MaybeAsyncJsCallbackExt},
};
use napi::bindgen_prelude::{Either, FnArgs};
use oxc::transformer::ESTarget;
use rolldown::{
  AddonOutputOption, AdvancedChunksOptions, AssetFilenamesOutputOption, BundlerOptions,
  ChunkFilenamesOutputOption, DeferSyncScanDataOption, HashCharacters, IsExternal, JsxPreset,
  MatchGroup, MatchGroupName, ModuleType, OptimizationOption, OutputExports, OutputFormat,
  Platform, RawMinifyOptions, SanitizeFilename, TransformOptions,
};
use rolldown_common::DeferSyncScanData;
use rolldown_plugin::__inner::SharedPluginable;
use rolldown_utils::indexmap::FxIndexMap;
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::FxHashMap;
use std::path::PathBuf;
use url::Url;

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
      Box::pin(async move {
        fn_js
          .await_call(FnArgs { data: (BindingRenderedChunk::new(chunk),) })
          .await
          .map_err(anyhow::Error::from)
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
        let chunk = (chunk.clone().into(),);
        Box::pin(async move {
          func.invoke_async(FnArgs { data: chunk }).await.map_err(anyhow::Error::from)
        })
      }))),
    })
    .transpose()
}

fn normalize_sanitize_filename(
  option: Option<SanitizeFileName>,
) -> napi::Result<Option<SanitizeFilename>> {
  option
    .map(move |value| match value {
      Either::A(value) => Ok(SanitizeFilename::Boolean(value)),
      Either::B(func) => Ok(SanitizeFilename::Fn(Arc::new(move |name| {
        let func = Arc::clone(&func);
        let name = name.to_string();
        Box::pin(async move {
          func.invoke_async(FnArgs { data: (name,) }).await.map_err(anyhow::Error::from)
        })
      }))),
    })
    .transpose()
}

fn normalize_asset_file_names_option(
  option: Option<AssetFileNamesOutputOption>,
) -> napi::Result<Option<AssetFilenamesOutputOption>> {
  option
    .map(move |value| match value {
      Either::A(str) => Ok(AssetFilenamesOutputOption::String(str)),
      Either::B(func) => Ok(AssetFilenamesOutputOption::Fn(Arc::new(move |asset| {
        let func = Arc::clone(&func);
        let asset = (asset.clone().into(),);
        Box::pin(async move {
          func.invoke_async(FnArgs { data: asset }).await.map_err(anyhow::Error::from)
        })
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
      Box::pin(async move { func.invoke_async((name,).into()).await.map_err(anyhow::Error::from) })
    })),
  })
}

fn normalize_es_target(target: Option<&Either<String, Vec<String>>>) -> ESTarget {
  target.map_or(ESTarget::ESNext, |target| {
    let targets = match target {
      Either::A(target) => {
        if target.contains(',') {
          target.split(',').collect::<Vec<&str>>()
        } else {
          vec![target.as_str()]
        }
      }
      Either::B(target) => target.iter().map(std::string::String::as_str).collect::<Vec<&str>>(),
    };
    for target in targets {
      if target.len() <= 2 || !target[..2].eq_ignore_ascii_case("es") {
        continue;
      }
      if target[2..].eq_ignore_ascii_case("next") {
        return ESTarget::ESNext;
      }
      if let Ok(n) = target[2..].parse::<usize>() {
        return match n {
          5 => ESTarget::ES5,
          6 | 2015 => ESTarget::ES2015,
          2016 => ESTarget::ES2016,
          2017 => ESTarget::ES2017,
          2018 => ESTarget::ES2018,
          2019 => ESTarget::ES2019,
          2020 => ESTarget::ES2020,
          2021 => ESTarget::ES2021,
          2022 => ESTarget::ES2022,
          2023 => ESTarget::ES2023,
          2024 => ESTarget::ES2024,
          _ => continue,
        };
      }
    }
    ESTarget::ES2015
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
          .invoke_async((source.to_string(), importer, is_resolved).into())
          .await
          .map_err(anyhow::Error::from)
      })
    })
  });

  let get_defer_sync_scan_data = input_options.defer_sync_scan_data.map(|ts_fn| {
    DeferSyncScanDataOption::new(move || {
      let ts_fn = Arc::clone(&ts_fn);
      Box::pin(async move {
        ts_fn
          .invoke_async(())
          .await
          .and_then(|ret| {
            ret.into_iter().map(TryInto::try_into).collect::<Result<Vec<DeferSyncScanData>, _>>()
          })
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
        ts_fn.invoke_async((source, sourcemap_path).into()).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let sourcemap_path_transform = output_options.sourcemap_path_transform.map(|ts_fn| {
    rolldown::SourceMapPathTransform::new(Arc::new(move |source, sourcemap_path| {
      let ts_fn = Arc::clone(&ts_fn);
      let source = source.to_string();
      let sourcemap_path = sourcemap_path.to_string();
      Box::pin(async move {
        ts_fn.invoke_async((source, sourcemap_path).into()).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let invalidate_js_side_cache = input_options.invalidate_js_side_cache.map(|ts_fn| {
    rolldown::InvalidateJsSideCache::new(Arc::new(move || {
      let ts_fn = Arc::clone(&ts_fn);
      Box::pin(async move { ts_fn.invoke_async((None,).into()).await.map_err(anyhow::Error::from) })
    }))
  });

  let mark_module_loaded = input_options.mark_module_loaded.map(|ts_fn| {
    rolldown::MarkModuleLoaded::new(Arc::new(move |module_id, success| {
      let ts_fn = Arc::clone(&ts_fn);
      let module_id = module_id.to_string();
      Box::pin(async move {
        ts_fn.invoke_async((module_id, success).into()).await.map_err(anyhow::Error::from)
      })
    }))
  });

  let on_log = input_options.on_log.map(|ts_fn| {
    rolldown::OnLog::new(Arc::new(move |level, log| {
      let ts_fn = Arc::clone(&ts_fn);
      Box::pin(async move {
        ts_fn
          .invoke_async((level.to_string(), log.into()).into())
          .await
          .map_err(anyhow::Error::from)
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

  let without_jsx = input_options.transform.as_ref().is_none_or(|v| v.jsx.is_none());
  let mut transform = if let Some(options) = input_options.transform {
    let es_target = normalize_es_target(options.target.as_ref());
    let is_preserve = matches!(&options.jsx, Some(Either::A(preset)) if preset == "preserve");
    let transform_options = oxc::transformer::TransformOptions::try_from(options)
      .map_err(|err| napi::Error::new(napi::Status::GenericFailure, err))?;

    let jsx_preset = if is_preserve {
      JsxPreset::Preserve
    } else if transform_options.jsx.jsx_plugin {
      JsxPreset::Enable
    } else {
      JsxPreset::Disable
    };

    TransformOptions::new(transform_options, es_target, jsx_preset)
  } else {
    TransformOptions::default()
  };

  if without_jsx {
    if let Some(jsx) = input_options.jsx {
      transform.jsx_preset = JsxPreset::Enable;
      match jsx {
        BindingJsx::Disable => {
          transform.jsx_preset = JsxPreset::Disable;
          transform.jsx.jsx_plugin = false;
        }
        BindingJsx::Preserve => {
          transform.jsx_preset = JsxPreset::Preserve;
          transform.jsx = oxc::transformer::JsxOptions::disable();
        }
        BindingJsx::React => {
          transform.jsx.runtime = oxc::transformer::JsxRuntime::Classic;
        }
        BindingJsx::ReactJsx => {}
      }
    }
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
    asset_filenames: normalize_asset_file_names_option(output_options.asset_file_names)?,
    entry_filenames: normalize_chunk_file_names_option(output_options.entry_file_names)?,
    chunk_filenames: normalize_chunk_file_names_option(output_options.chunk_file_names)?,
    css_entry_filenames: normalize_chunk_file_names_option(output_options.css_entry_file_names)?,
    css_chunk_filenames: normalize_chunk_file_names_option(output_options.css_chunk_file_names)?,
    sanitize_filename: normalize_sanitize_filename(output_options.sanitize_file_name)?,
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
    sourcemap_base_url: output_options.sourcemap_base_url.map(|maybe_url| {
      if let Ok(mut url) = Url::parse(&maybe_url) {
        if !url.path().ends_with('/') {
          url.set_path(&rolldown_utils::concat_string!(url.path(), "/"));
        }
        return url.to_string();
      }
      panic!("Invalid sourcemapBaseUrl: {maybe_url}");
    }),
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
    experimental: if let Some(experimental) = input_options.experimental {
      Some(experimental.try_into()?)
    } else {
      None
    },
    minify: output_options
      .minify
      .map(|opts| match opts {
        napi::bindgen_prelude::Either3::A(opts) => Ok(opts.into()),
        napi::bindgen_prelude::Either3::B(opts) => {
          if opts == "dce-only" {
            Ok(RawMinifyOptions::DeadCodeEliminationOnly)
          } else {
            Err(napi::Error::new(napi::Status::InvalidArg, "Invalid minify option"))
          }
        }
        napi::bindgen_prelude::Either3::C(opts) => Ok(opts.into()),
      })
      .transpose()?,
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
      min_module_size: inner.min_module_size,
      max_module_size: inner.max_module_size,
      max_size: inner.max_size,
      groups: inner.groups.map(|inner| {
        inner
          .into_iter()
          .map(|item| MatchGroup {
            name: match item.name {
              Either::A(name) => MatchGroupName::Static(name),
              Either::B(func) => {
                let func = Arc::clone(&func);
                MatchGroupName::Dynamic(Arc::new(move |module_id, ctx| {
                  let module_id = module_id.to_string();
                  let func = Arc::clone(&func);
                  let owned_ctx = ctx.clone();
                  Box::pin(async move {
                    func
                      .invoke_async((module_id, BindingChunkingContext::new(owned_ctx)).into())
                      .await
                      .map_err(anyhow::Error::from)
                  })
                }))
              }
            },
            test: item.test.map(|inner| match inner {
              Either::A(reg) => {
                rolldown::MatchGroupTest::Regex(reg.try_into().expect("Invalid regex pass to test"))
              }
              Either::B(func) => rolldown::MatchGroupTest::Function(Arc::new(move |id: &str| {
                let id = id.to_string();
                let func = Arc::clone(&func);
                Box::pin(async move {
                  func.invoke_async((id,).into()).await.map_err(anyhow::Error::from)
                })
              })),
            }),
            priority: item.priority,
            min_size: item.min_size,
            min_share_count: item.min_share_count,
            max_module_size: item.max_module_size,
            min_module_size: item.min_module_size,
            max_size: item.max_size,
          })
          .collect::<Vec<_>>()
      }),
      include_dependencies_recursively: None,
    }),
    checks: input_options.checks.map(Into::into),
    profiler_names: input_options.profiler_names,
    watch: input_options.watch.map(TryInto::try_into).transpose()?,
    legal_comments: output_options
      .legal_comments
      .map(|inner| match inner.as_str() {
        "none" => Ok(rolldown::LegalComments::None),
        "inline" => Ok(rolldown::LegalComments::Inline),
        _ => Err(napi::Error::new(
          napi::Status::GenericFailure,
          format!("Invalid value for `legalComments` option: {inner}"),
        )),
      })
      .transpose()?,
    drop_labels: input_options.drop_labels,
    keep_names: input_options.keep_names,
    polyfill_require: output_options.polyfill_require,
    defer_sync_scan_data: get_defer_sync_scan_data,
    transform: Some(transform),
    make_absolute_externals_relative: input_options
      .make_absolute_externals_relative
      .map(Into::into),
    debug: input_options.debug.map(|inner| rolldown::DebugOptions { session_id: inner.session_id }),
    invalidate_js_side_cache,
    mark_module_loaded,
    log_level: Some(input_options.log_level.into()),
    on_log,
    preserve_modules: output_options.preserve_modules,
    virtual_dirname: output_options.virtual_dirname,
    preserve_modules_root: output_options.preserve_modules_root,
    preserve_entry_signatures: input_options
      .preserve_entry_signatures
      .map(std::convert::TryInto::try_into)
      .transpose()?,
    optimization: input_options.optimization.map(OptimizationOption::from),
    top_level_var: output_options.top_level_var,
    minify_internal_exports: output_options.minify_internal_exports,
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
