use rolldown_common::ResolvedPath;
use rolldown_plugin::HookLoadArgs;
use rolldown_sourcemap::SourceMap;
use sugar_path::AsPath;

use crate::{error::BatchedErrors, plugin_driver::PluginDriver};

pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  fs: &dyn rolldown_fs::FileSystem,
  sourcemap_chain: &mut Vec<SourceMap>,
) -> Result<String, BatchedErrors> {
  let source =
    if let Some(r) = plugin_driver.load(&HookLoadArgs { id: &resolved_path.path }).await? {
      if let Some(map) = r.map {
        sourcemap_chain.push(map);
      }
      r.code
    } else if resolved_path.ignored {
      String::new()
    } else {
      fs.read_to_string(resolved_path.path.as_path())?
    };
  Ok(source)
}
