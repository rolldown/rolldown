pub mod module_loader;
pub mod module_task;
mod runtime_module_task;
pub mod task_context;
pub use module_loader::ModuleLoader;
pub mod deferred_scan_data;
pub mod external_module_task;
pub mod resolve_utils;

use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use rolldown_error::BuildResult;

/// A task that panics never reports back to the loader, which would then wait forever.
async fn catch_task_panic(task: impl Future<Output = BuildResult<()>>) -> BuildResult<()> {
  AssertUnwindSafe(task).catch_unwind().await.unwrap_or_else(|payload| {
    let message = payload
      .downcast_ref::<&str>()
      .copied()
      .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
      .unwrap_or("unknown panic");
    Err(anyhow::anyhow!("A module task panicked: {message}").into())
  })
}
