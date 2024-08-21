pub mod module_loader;
mod module_task;
mod runtime_module_task;
pub mod task_context;
mod task_result;

pub use module_loader::ModuleLoader;
use rolldown_error::BuildDiagnostic;

use self::{runtime_module_task::RuntimeModuleTaskResult, task_result::NormalModuleTaskResult};
pub enum Msg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeModuleTaskResult),
  BuildErrors(Vec<BuildDiagnostic>),
  Panics(anyhow::Error),
}
