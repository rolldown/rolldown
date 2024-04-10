#[allow(clippy::module_inception)]
pub mod module_loader;
mod normal_module_task;
mod runtime_normal_module_task;
pub mod task_context;
mod task_result;

pub use module_loader::ModuleLoader;

use crate::error::BatchedErrors;

use self::{
  runtime_normal_module_task::RuntimeNormalModuleTaskResult, task_result::NormalModuleTaskResult,
};
pub enum Msg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeNormalModuleTaskResult),
  BuildErrors(BatchedErrors),
  Panics(anyhow::Error),
}
