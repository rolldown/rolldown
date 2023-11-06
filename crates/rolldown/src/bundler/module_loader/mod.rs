#[allow(clippy::module_inception)]
mod module_loader;
mod module_task_context;
mod normal_module_task;
mod runtime_normal_module_task;
mod task_result;

pub use module_loader::ModuleLoader;

use self::task_result::NormalModuleTaskResult;
pub enum Msg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(NormalModuleTaskResult),
}
