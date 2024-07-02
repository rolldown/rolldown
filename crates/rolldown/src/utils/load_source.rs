use std::sync::Arc;

use anyhow::Context;
use rolldown_common::{side_effects::HookSideEffects, ModuleType, ResolvedPath};
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::mime::guess_mime;
use sugar_path::SugarPath;

pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  module_type: ModuleType,
  fs: &dyn rolldown_fs::FileSystem,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
) -> anyhow::Result<String> {
  let source = if let Some(r) =
    plugin_driver.load(&HookLoadArgs { id: &resolved_path.path }).await?
  {
    if let Some(map) = r.map {
      sourcemap_chain.push(map);
    }
    if let Some(v) = r.side_effects {
      *side_effects = Some(v);
    }
    r.code
  } else if resolved_path.ignored {
    String::new()
  } else {
    match module_type {
      ModuleType::Base64 | ModuleType::Binary => {
        rolldown_utils::base64::to_standard_base64(fs.read(resolved_path.path.as_path())?)
      }
      ModuleType::Dataurl => {
        let data =
          fs.read(resolved_path.path.as_path()).with_context(|| Arc::clone(&resolved_path.path))?;
        let mime = guess_mime(resolved_path.path.as_path(), &data)?;
        let is_plain_text = mime.type_() == mime::TEXT;
        if is_plain_text {
          let text = String::from_utf8(data)?;
          let text = urlencoding::encode(&text);
          // TODO: should we support non-utf8 text?
          format!("data:{mime};charset=utf-8,{text}")
        } else {
          let encoded = rolldown_utils::base64::to_url_safe_base64(&data);
          format!("data:{mime};base64,{encoded}")
        }
      }
      _ => fs.read_to_string(resolved_path.path.as_path())?,
    }
  };
  Ok(source)
}
