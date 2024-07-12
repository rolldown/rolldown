mod css_module_task;
mod ecma_module_task;
pub mod module_loader;
mod runtime_ecma_module_task;
pub mod task_context;
mod task_result;

pub use module_loader::ModuleLoader;
use rolldown_error::BuildError;

use self::{
  runtime_ecma_module_task::RuntimeEcmaModuleTaskResult, task_result::NormalModuleTaskResult,
};
pub enum Msg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeEcmaModuleTaskResult),
  BuildErrors(Vec<BuildError>),
  Panics(anyhow::Error),
}
