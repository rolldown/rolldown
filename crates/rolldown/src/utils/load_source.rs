use rolldown_common::{
  side_effects::HookSideEffects, ModuleType, NormalizedBundlerOptions, ResolvedId, StrOrBytes,
};
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use sugar_path::SugarPath;

pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_id: &ResolvedId,
  fs: &dyn rolldown_fs::FileSystem,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<(StrOrBytes, Option<ModuleType>)> {
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
    (Some(source), Some(module_type)) => Ok((source.into(), Some(module_type))),
    (Some(source), None) => {
      // The `load` hook returns content without specifying the module type, we consider it as `Js` by default.
      // - This makes the behavior consistent with Rollup.
      // - It's also not friendly to make users have to specify the module type in the `load` hook.
      // - Why don't we jump into the guessing logic? Because guessing by extension is not reliable in the rollup's ecosystem.
      Ok((source.into(), Some(ModuleType::Js)))
    }
    (None, None) => {
      let guessed = resolved_id
        .id
        .as_path()
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| options.module_types.get(ext).cloned());
      if let Some(guessed) = guessed {
        match &guessed {
          ModuleType::Base64 | ModuleType::Binary | ModuleType::Dataurl => {
            Ok((StrOrBytes::Bytes(fs.read(resolved_id.id.as_path())?), Some(guessed)))
          }
          ModuleType::Js
          | ModuleType::Jsx
          | ModuleType::Ts
          | ModuleType::Tsx
          | ModuleType::Json
          | ModuleType::Text
          | ModuleType::Empty
          | ModuleType::Css
          | ModuleType::Custom(_) => {
            Ok((StrOrBytes::Str(fs.read_to_string(resolved_id.id.as_path())?), Some(guessed)))
          }
        }
      } else {
        Err(anyhow::format_err!("Fail to guess module type for {:?}. So rolldown could load this asset correctly. Please use the load hook to load the resource", resolved_id.id))
      }
    }
    (None, Some(_)) => unreachable!("Invalid state"),
  }
}
