use std::{borrow::Cow, path::Path};

use rolldown_common::{
  ModuleType, NormalizedBundlerOptions, ResolvedId, StrOrBytes, side_effects::HookSideEffects,
};
use rolldown_fs::FileSystem;
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

#[expect(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn load_source<Fs: FileSystem + 'static>(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  fs: Fs,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
  options: &NormalizedBundlerOptions,
  asserted_module_type: Option<&ModuleType>,
  is_read_from_disk: &mut bool,
) -> anyhow::Result<(StrOrBytes, ModuleType)> {
  let (maybe_source, maybe_module_type) =
    match plugin_driver.load(&HookLoadArgs { id: &resolved_id.id }).await? {
      Some(load_hook_output) => {
        sourcemap_chain.extend(load_hook_output.map);
        if let Some(v) = load_hook_output.side_effects {
          *side_effects = Some(v);
        }

        (Some(load_hook_output.code.to_string()), load_hook_output.module_type)
      }
      _ => {
        if resolved_id.ignored {
          (Some(String::new()), Some(ModuleType::Empty))
        } else {
          (None, None)
        }
      }
    };

  if let Some(asserted) = asserted_module_type {
    let is_type_conflicted = match &maybe_module_type {
      None => false,
      Some(user_specified_type) if user_specified_type == asserted => false,
      _ => true,
    };
    if is_type_conflicted {
      // TODO: emit a warning about the type conflict
    }
  }
  if maybe_source.is_some() {
    *is_read_from_disk = false;
  }

  match (maybe_source, maybe_module_type) {
    (Some(source), Some(module_type)) => Ok((source.into(), module_type)),
    (source, None) => {
      let guessed = get_module_loader_from_file_extension(&resolved_id.id, &options.module_types);
      match (source, guessed) {
        (None, None) => {
          // - Unknown module type,
          // - No loader to load corresponding module
          // - User don't specify moduleTypeMapping, we treated it as JS
          Ok((
            StrOrBytes::Str({
              #[cfg(not(target_family = "wasm"))]
              {
                let id = resolved_id.id.clone();
                tokio::runtime::Handle::current()
                  .spawn_blocking(move || fs.read_to_string(id.as_path()))
                  .await??
              }
              #[cfg(target_family = "wasm")]
              {
                fs.read_to_string(resolved_id.id.as_path())?
              }
            }),
            ModuleType::Js,
          ))
        }
        (source, Some(guessed)) => match &guessed {
          ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl | ModuleType::Asset => {
            Ok((
              StrOrBytes::Bytes({
                source
                  .map(String::into_bytes)
                  .ok_or(())
                  .or_else(|()| fs.read(resolved_id.id.as_path()))?
              }),
              guessed,
            ))
          }
          ModuleType::Js
          | ModuleType::Jsx
          | ModuleType::Ts
          | ModuleType::Tsx
          | ModuleType::Json
          | ModuleType::Text
          | ModuleType::Empty
          | ModuleType::Css
          | ModuleType::Custom(_) => Ok((
            StrOrBytes::Str({
              if let Some(s) = source {
                s
              } else {
                #[cfg(not(target_family = "wasm"))]
                {
                  let id = resolved_id.id.clone();
                  tokio::runtime::Handle::current()
                    .spawn_blocking(move || fs.read_to_string(id.as_path()))
                    .await??
                }
                #[cfg(target_family = "wasm")]
                {
                  fs.read_to_string(resolved_id.id.as_path())?
                }
              }
            }),
            guessed,
          )),
        },
        (Some(source), None) => Ok((StrOrBytes::Str(source), ModuleType::Js)),
      }
    }
    (None, Some(ty)) => {
      if asserted_module_type.is_some() {
        Ok((read_file_by_module_type(resolved_id.id.as_path(), &ty, &fs)?, ty))
      } else {
        unreachable!("Invalid state")
      }
    }
  }
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

fn read_file_by_module_type<Fs: FileSystem>(
  path: impl AsRef<Path>,
  ty: &ModuleType,
  fs: &Fs,
) -> anyhow::Result<StrOrBytes> {
  let path = path.as_ref();
  match ty {
    ModuleType::Js
    | ModuleType::Jsx
    | ModuleType::Ts
    | ModuleType::Tsx
    | ModuleType::Json
    | ModuleType::Css
    | ModuleType::Empty
    | ModuleType::Custom(_)
    | ModuleType::Text => Ok(StrOrBytes::Str(fs.read_to_string(path)?)),
    ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl | ModuleType::Asset => {
      Ok(StrOrBytes::Bytes(fs.read(path)?))
    }
  }
}
