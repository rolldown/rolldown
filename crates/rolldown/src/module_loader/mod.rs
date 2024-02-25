#[allow(clippy::module_inception)]
pub mod module_loader;
pub mod module_task_context;
mod normal_module_task;
mod runtime_normal_module_task;
mod task_result;

pub use module_loader::ModuleLoader;

use crate::error::BatchedErrors;

use self::{
  runtime_normal_module_task::RuntimeNormalModuleTaskResult, task_result::NormalModuleTaskResult,
};
pub enum Msg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeNormalModuleTaskResult),
  Errors(BatchedErrors),
}
