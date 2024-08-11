use rolldown_common::{
  side_effects::HookSideEffects, ModuleType, NormalizedBundlerOptions, ResolvedId, StrOrBytes,
};
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::path_ext::clean_url;
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
      // Considering path with `?/#`
      let cleaned_id = clean_url(&resolved_id.id);
      let ext = cleaned_id.as_path().extension().and_then(|ext| ext.to_str());
      let guessed = ext.and_then(|ext| options.module_types.get(ext).cloned());
      match (source, guessed) {
        (None, None) => Ok((
          StrOrBytes::Str(fs.read_to_string(resolved_id.id.as_path())?),
          ModuleType::Custom(ext.map(String::from).unwrap_or_default()),
        )),
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
