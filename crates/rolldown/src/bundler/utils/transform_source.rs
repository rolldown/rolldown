use rolldown_fs::FileSystem;

use crate::{bundler::plugin_driver::PluginDriver, error::BatchedErrors, HookTransformArgs};

pub async fn transform_source<T: FileSystem + Default + 'static>(
  plugin_driver: &PluginDriver<T>,
  mut source: String,
) -> Result<String, BatchedErrors> {
  if let Some(r) =
    plugin_driver.transform(&HookTransformArgs { id: source.as_ref(), code: &source }).await?
  {
    source = r.code;
  };

  Ok(source)
}
