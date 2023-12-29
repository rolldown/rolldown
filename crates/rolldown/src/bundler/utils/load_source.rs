use rolldown_common::FilePath;
use rolldown_fs::FileSystem;
use sugar_path::AsPath;

use crate::{bundler::plugin_driver::PluginDriver, error::BatchedErrors, HookLoadArgs};

pub async fn load_source<T: FileSystem + Default + 'static>(
  plugin_driver: &PluginDriver<T>,
  path: &FilePath,
  fs: &T,
) -> Result<String, BatchedErrors> {
  let source = if let Some(r) = plugin_driver.load(&HookLoadArgs { id: path.as_ref() }).await? {
    r.code
  } else {
    fs.read_to_string(path.as_path())?
  };
  Ok(source)
}
