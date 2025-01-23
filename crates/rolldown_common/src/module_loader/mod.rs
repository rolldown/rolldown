use arcstr::ArcStr;
use rolldown_error::BuildDiagnostic;
use runtime_task_result::RuntimeModuleTaskResult;
use task_result::NormalModuleTaskResult;

use crate::{EmittedChunk, ResolvedId};

pub mod runtime_module_brief;
pub mod runtime_task_result;
pub mod task_result;

pub enum ModuleLoaderMsg {
  NormalModuleDone(NormalModuleTaskResult),
  RuntimeNormalModuleDone(RuntimeModuleTaskResult),
  FetchModule(ResolvedId),
  AddEntryModule(AddEntryModuleMsg),
  BuildErrors(Vec<BuildDiagnostic>),
}

pub struct AddEntryModuleMsg {
  pub chunk: EmittedChunk,
  pub reference_id: ArcStr,
}
