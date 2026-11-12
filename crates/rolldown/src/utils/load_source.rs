use std::{borrow::Cow, path::Path};

use rolldown_common::{
  EmittedAsset, ModuleIdx, ModuleType, NormalizedBundlerOptions, ResolvedId, SourcemapChainElement,
  StrOrBytes, side_effects::HookSideEffects,
};
use rolldown_fs::FileSystem;
use rolldown_plugin::{HookLoadArgs, PluginDriver, SharedModuleCacheRecorder};
use rolldown_utils::futures::spawn_blocking;
use rolldown_utils::url::clean_url;
use rustc_hash::FxHashMap;

#[expect(clippy::too_many_arguments)]
pub async fn load_source<Fs: FileSystem + Clone + 'static>(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  fs: Fs,
  sourcemap_chain: &mut Vec<SourcemapChainElement>,
  side_effects: &mut Option<HookSideEffects>,
  declared_module_size: &mut Option<usize>,
  options: &NormalizedBundlerOptions,
  asserted_module_type: Option<&ModuleType>,
  is_read_from_disk: &mut bool,
  module_idx: ModuleIdx,
  cache_recorder: Option<&SharedModuleCacheRecorder>,
  // The contents of the module file, when the persistent cache already read them for key 1.
  // Using them makes key 1 match what is loaded, also if the file changes during the build.
  preloaded: Option<Vec<u8>>,
) -> anyhow::Result<(StrOrBytes, ModuleType)> {
  let raw_id = resolved_id.id.raw_id();
  let (maybe_source, mut maybe_module_type) = if resolved_id.id.is_empty_module() {
    (Some(String::new()), Some(ModuleType::Empty))
  } else {
    plugin_driver
      .load(
        &HookLoadArgs {
          id: &resolved_id.id,
          raw_id,
          attributes: &resolved_id.attributes,
          module_idx,
          asserted_module_type,
        },
        cache_recorder,
      )
      .await?
      .map(|load_hook_output| {
        sourcemap_chain.extend(load_hook_output.map.map(SourcemapChainElement::Load));
        if let Some(v) = load_hook_output.side_effects {
          plugin_driver.set_hook_side_effects(&resolved_id.id, *side_effects, v);
          *side_effects = Some(v);
        }
        if let Some(v) = load_hook_output.declared_module_size {
          *declared_module_size = Some(v);
        }
        (Some(load_hook_output.code.to_string()), load_hook_output.module_type)
      })
      .unwrap_or_default()
  };

  emit_new_url_asset_if_missing(
    plugin_driver,
    options,
    resolved_id,
    asserted_module_type,
    fs.clone(),
  )
  .await?;
  // A `type` attribute with a built-in meaning beats the file extension, but not the plugin.
  let attributes_module_type = resolved_id.attributes.module_type();
  if maybe_module_type.is_none() {
    maybe_module_type = attributes_module_type.clone();
  }
  if maybe_source.is_some() {
    *is_read_from_disk = false;
  }

  match (maybe_source, maybe_module_type) {
    (Some(source), Some(module_type)) => Ok((source.into(), module_type)),
    (source, None) => {
      let guessed = get_module_loader_from_file_extension(raw_id, &options.module_types);
      match (source, guessed) {
        (None, None) => {
          // - Unknown module type,
          // - No loader to load corresponding module
          // - User don't specify moduleTypeMapping, we treated it as JS
          Ok((StrOrBytes::Str(read_string(raw_id, fs, preloaded).await?), ModuleType::Js))
        }
        (source, Some(guessed)) => match &guessed {
          ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl => Ok((
            StrOrBytes::Bytes(match source {
              Some(s) => s.into_bytes(),
              None => read_bytes(raw_id, fs, preloaded).await?,
            }),
            guessed,
          )),
          ModuleType::Copy => Err(anyhow::format_err!(
            "Encountered a module with type `copy`, but no plugin handled it. \
               If you configured this file's extension as `copy` in `moduleTypes`, \
               ensure the builtin copy-module plugin is enabled."
          ))?,
          ModuleType::Asset => Err(anyhow::format_err!(
            "Encountered a module with type `asset`, but no plugin handled it. \
               If you configured this file's extension as `asset` in `moduleTypes`, \
               ensure the builtin asset-module plugin is enabled."
          ))?,
          ModuleType::Js
          | ModuleType::Jsx
          | ModuleType::Ts
          | ModuleType::Tsx
          | ModuleType::Json
          | ModuleType::Text
          | ModuleType::Empty
          | ModuleType::Css
          | ModuleType::Custom(_) => Ok((
            StrOrBytes::Str(match source {
              Some(s) => s,
              None => read_string(raw_id, fs, preloaded).await?,
            }),
            guessed,
          )),
        },
        (Some(source), None) => Ok((StrOrBytes::Str(source), ModuleType::Js)),
      }
    }
    (None, Some(ty)) => {
      assert!(asserted_module_type.is_some() || attributes_module_type.is_some(), "Invalid state");
      Ok((read_file_by_module_type(Path::new(raw_id), &ty, fs, preloaded).await?, ty))
    }
  }
}

/// A plugin can load a `new URL()` target as code; the URL still needs its file.
/// See internal-docs/plugin-asset-module/implementation.md
pub async fn emit_new_url_asset_if_missing<Fs: FileSystem + 'static>(
  plugin_driver: &PluginDriver,
  options: &NormalizedBundlerOptions,
  resolved_id: &ResolvedId,
  asserted_module_type: Option<&ModuleType>,
  fs: Fs,
) -> anyhow::Result<()> {
  if matches!(asserted_module_type, Some(ModuleType::Asset))
    && resolved_id.id.is_path()
    && fs.exists(Path::new(clean_url(resolved_id.id.raw_id())))
    && plugin_driver.file_emitter.file_ref_for_module(&resolved_id.id).is_none()
  {
    emit_new_url_asset(plugin_driver, options, &resolved_id.id, fs).await?;
  }
  Ok(())
}

/// Emit `id` as an asset and associate it with the module, as the builtin asset-module plugin
/// would have done if a user plugin had not loaded the module first.
async fn emit_new_url_asset<Fs: FileSystem + 'static>(
  plugin_driver: &PluginDriver,
  options: &NormalizedBundlerOptions,
  id: &str,
  fs: Fs,
) -> anyhow::Result<()> {
  let clean_id = clean_url(id);
  // The file is read from disk here, so watch it like the asset plugin does.
  plugin_driver.watch_files.insert(clean_id.into());
  let path = Path::new(clean_id).to_path_buf();
  let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("asset").to_string();
  let original_file_name =
    path.strip_prefix(&options.cwd).unwrap_or(&path).to_string_lossy().into_owned();
  let source = read_file_by_module_type(&path, &ModuleType::Binary, fs, None).await?;
  let file = EmittedAsset {
    name: Some(file_name),
    original_file_name: Some(original_file_name),
    source,
    ..Default::default()
  };
  let asset_filename = options.asset_filename_with_file(&file).await?;
  let sanitized_file_name = options.sanitize_file_name_with_file(&file).await?;
  let reference_id =
    plugin_driver.file_emitter.emit_file(file, asset_filename, sanitized_file_name)?;
  plugin_driver.file_emitter.associate_module_with_file_ref(id, &reference_id);
  Ok(())
}

/// ref: https://github.com/evanw/esbuild/blob/9c13ae1f06dfa909eb4a53882e3b7e4216a503fe/internal/bundler/bundler.go#L1161-L1183
fn get_module_loader_from_file_extension<S: AsRef<str>>(
  id: S,
  module_types: &FxHashMap<Cow<'static, str>, ModuleType>,
) -> Option<ModuleType> {
  let id = id.as_ref();
  for i in memchr::memchr_iter(b'.', id.as_bytes()) {
    if let Some(ty) = module_types.get(&id[i + 1..]) {
      return Some(ty.clone());
    }
  }
  None
}

async fn read_string<Fs: FileSystem + 'static>(
  raw_id: &str,
  fs: Fs,
  preloaded: Option<Vec<u8>>,
) -> anyhow::Result<String> {
  if let Some(bytes) = preloaded {
    return String::from_utf8(bytes).map_err(|_| {
      std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")
        .into()
    });
  }
  #[cfg(not(target_family = "wasm"))]
  {
    let raw_id = raw_id.to_owned();
    Ok(spawn_blocking(move || fs.read_to_string(Path::new(&raw_id))).await??)
  }
  #[cfg(target_family = "wasm")]
  {
    Ok(fs.read_to_string(Path::new(raw_id))?)
  }
}

async fn read_bytes<Fs: FileSystem + 'static>(
  raw_id: &str,
  fs: Fs,
  preloaded: Option<Vec<u8>>,
) -> anyhow::Result<Vec<u8>> {
  if let Some(bytes) = preloaded {
    return Ok(bytes);
  }
  if cfg!(target_family = "wasm") {
    Ok(fs.read(Path::new(raw_id))?)
  } else {
    let raw_id = raw_id.to_owned();
    Ok(spawn_blocking(move || fs.read(Path::new(&raw_id))).await??)
  }
}

async fn read_file_by_module_type<Fs: FileSystem + 'static>(
  path: impl AsRef<Path>,
  ty: &ModuleType,
  fs: Fs,
  preloaded: Option<Vec<u8>>,
) -> anyhow::Result<StrOrBytes> {
  if let Some(bytes) = preloaded.filter(|_| !matches!(ty, ModuleType::Asset)) {
    return match ty {
      ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl => Ok(StrOrBytes::Bytes(bytes)),
      _ => Ok(StrOrBytes::Str(read_string("", fs, Some(bytes)).await?)),
    };
  }
  let path = path.as_ref().to_path_buf();
  match ty {
    ModuleType::Js
    | ModuleType::Jsx
    | ModuleType::Ts
    | ModuleType::Tsx
    | ModuleType::Json
    | ModuleType::Css
    | ModuleType::Empty
    | ModuleType::Copy
    | ModuleType::Custom(_)
    | ModuleType::Text => Ok(StrOrBytes::Str({
      if cfg!(target_family = "wasm") {
        fs.read_to_string(&path)?
      } else {
        spawn_blocking(move || fs.read_to_string(&path)).await??
      }
    })),
    ModuleType::Asset => Err(anyhow::format_err!(
      "Encountered a module with type `asset` in read_file_by_module_type. \
         Asset modules should be handled by the builtin asset-module plugin."
    ))?,
    ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl => Ok(StrOrBytes::Bytes({
      if cfg!(target_family = "wasm") {
        fs.read(&path)?
      } else {
        spawn_blocking(move || fs.read(&path)).await??
      }
    })),
  }
}
