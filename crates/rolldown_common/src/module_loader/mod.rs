use arcstr::ArcStr;
use rolldown_error::BuildDiagnostic;
use runtime_task_result::RuntimeModuleTaskResult;
use task_result::NormalModuleTaskResult;

use crate::ResolvedId;

pub mod runtime_module_brief;
pub mod runtime_task_result;
pub mod task_result;

pub enum ModuleLoaderMsg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeModuleTaskResult),
  FetchModule(ResolvedId),
  AddEntryModule(ExtraEntryModuleData),
  BuildErrors(Vec<BuildDiagnostic>),
}

pub struct ExtraEntryModuleData {
  pub file_name: Option<ArcStr>,
  pub name: Option<ArcStr>,
  pub resolved_id: ResolvedId,
}
