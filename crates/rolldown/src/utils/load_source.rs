use rolldown_common::{
  side_effects::HookSideEffects, ModuleType, NormalizedBundlerOptions, ResolvedId, StrOrBytes,
};
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  fs: &dyn rolldown_fs::FileSystem,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<(StrOrBytes, ModuleType)> {
  let (maybe_source, maybe_module_type) = if let Some(load_hook_output) =
    plugin_driver.load(&HookLoadArgs { id: &resolved_id.id }).await?
  {
    sourcemap_chain.extend(load_hook_output.map);
    if let Some(v) = load_hook_output.side_effects {
      *side_effects = Some(v);
    }

    (Some(load_hook_output.code), load_hook_output.module_type)
  } else if resolved_id.ignored {
    (Some(String::new()), Some(ModuleType::Js))
  } else {
    (None, None)
  };

  match (maybe_source, maybe_module_type) {
    (Some(source), Some(module_type)) => Ok((source.into(), module_type)),
    (source, None) => {
      let guessed = get_module_loader_from_file_extension(&resolved_id.id, &options.module_types);
      match (source, guessed) {
        (None, None) => {
          // - Unknown module type,
          // - No loader to load corresponding module
          // - User don't specify moduleTypeMapping, we treated it as JS
          Ok((StrOrBytes::Str(fs.read_to_string(resolved_id.id.as_path())?), ModuleType::Js))
        }
        (source, Some(guessed)) => match &guessed {
          ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl => Ok((
            StrOrBytes::Bytes({
              source
                .map(String::into_bytes)
                .ok_or(())
                .or_else(|()| fs.read(resolved_id.id.as_path()))?
            }),
            guessed,
          )),
          ModuleType::Js
          | ModuleType::Jsx
          | ModuleType::Ts
          | ModuleType::Tsx
          | ModuleType::Json
          | ModuleType::Text
          | ModuleType::Empty
          | ModuleType::Css
          | ModuleType::Custom(_) => Ok((
            StrOrBytes::Str(
              source.ok_or(()).or_else(|()| fs.read_to_string(resolved_id.id.as_path()))?,
            ),
            guessed,
          )),
        },
        (Some(source), None) => Ok((StrOrBytes::Str(source), ModuleType::Js)),
      }
    }
    (None, Some(_)) => unreachable!("Invalid state"),
  }
}

/// ref: https://github.com/evanw/esbuild/blob/9c13ae1f06dfa909eb4a53882e3b7e4216a503fe/internal/bundler/bundler.go#L1161-L1183
fn get_module_loader_from_file_extension<S: AsRef<str>>(
  id: S,
  module_types: &FxHashMap<String, ModuleType>,
) -> Option<ModuleType> {
  let id = id.as_ref();
  for i in memchr::memchr_iter(b'.', id.as_bytes()) {
    if let Some(ty) = module_types.get(&id[i + 1..]) {
      return Some(ty.clone());
    }
  }
  None
}
