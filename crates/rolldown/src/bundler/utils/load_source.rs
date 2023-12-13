use rolldown_common::FilePath;
use sugar_path::AsPath;

use crate::{bundler::plugin_driver::PluginDriver, error::BatchedErrors, HookLoadArgs};

pub async fn load_source(
  plugin_driver: &PluginDriver,
  path: &FilePath,
  fs: &dyn rolldown_fs::FileSystem,
) -> Result<String, BatchedErrors> {
  let source = if let Some(r) = plugin_driver.load(&HookLoadArgs { id: path.as_ref() }).await? {
    r.code
  } else if path.is_ignored() {
    String::new()
  } else {
    fs.read_to_string(path.as_path())?
  };
  Ok(source)
}
