#[allow(clippy::module_inception)]
mod module_loader;
mod module_task;
mod task_result;

pub use module_loader::ModuleLoader;

use self::task_result::TaskResult;
pub enum Msg {
  Done(TaskResult),
}
