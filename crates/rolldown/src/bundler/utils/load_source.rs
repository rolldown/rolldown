use rolldown_common::ResolvedPath;
use sugar_path::AsPath;

use crate::{
  bundler::plugin_driver::PluginDriver, error::BatchedErrors, HookLoadArgs, HookLoadOutput,
};

pub async fn load_source(
  plugin_driver: &PluginDriver,
  resolved_path: &ResolvedPath,
  fs: &dyn rolldown_fs::FileSystem,
) -> Result<HookLoadOutput, BatchedErrors> {
  let value = if let Some(r) = plugin_driver.load(&HookLoadArgs { id: &resolved_path.path }).await?
  {
    r
  } else if resolved_path.ignored {
    HookLoadOutput { code: String::new(), map: None }
  } else {
    HookLoadOutput { code: fs.read_to_string(resolved_path.path.as_path())?, map: None }
  };
  Ok(value)
}
