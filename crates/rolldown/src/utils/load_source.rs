use rolldown_common::{side_effects::HookSideEffects, ModuleType, ResolvedPath};
use rolldown_plugin::{HookLoadArgs, PluginDriver};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::{url_encoding::url_encode, mime::{get_data_url_mime_by_extension, get_data_url_mime_by_data}};
use sugar_path::SugarPath;


pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  module_type: ModuleType,
  fs: &dyn rolldown_fs::FileSystem,
  sourcemap_chain: &mut Vec<SourceMap>,
  side_effects: &mut Option<HookSideEffects>,
) -> anyhow::Result<String> {
  let source =
    if let Some(r) = plugin_driver.load(&HookLoadArgs { id: &resolved_path.path }).await? {
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
        ModuleType::DataUrl => {
          // let extension: &str = resolved_path.path.extension().unwrap().to_str().unwrap(); DO NOT USE `extension` method
          let extension: &str = resolved_path.path.split('.').last().unwrap();
          let mime = get_data_url_mime_by_extension(extension).unwrap_or_else(|| {
            let data = fs.read(resolved_path.path.as_path()).expect("Failed to read data.");
            get_data_url_mime_by_data(&data).expect("Failed to infer mime type from data.")
          });
          // If you cannot infer mime type with ext, try to infer with data

          let content: String = if matches!(mime.type_(), mime::TEXT) {
            let content = fs.read_to_string(resolved_path.path.as_path())?;
            url_encode(&content)
          } else {
            let content =
              rolldown_utils::base64::to_url_safe_base64(fs.read(resolved_path.path.as_path())?);
            ["base64,", &content].concat()
          };
          format!("data:{mime};{content}")
        }
        _ => fs.read_to_string(resolved_path.path.as_path())?,
      }
    };
  Ok(source)
}
