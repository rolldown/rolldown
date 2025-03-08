use std::{fmt::Display, sync::Arc};

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

impl Display for ModuleLoaderMsg {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ModuleLoaderMsg::NormalModuleDone(normal_module_task_result) => {
        f.write_str("NormalModuleDone")
      }
      ModuleLoaderMsg::RuntimeNormalModuleDone(runtime_module_task_result) => {
        f.write_str("RuntimeNormalModuleDone")
      }
      ModuleLoaderMsg::FetchModule(resolved_id) => f.write_str("FetchModule"),
      ModuleLoaderMsg::AddEntryModule(add_entry_module_msg) => f.write_str("AddEntryModule"),
      ModuleLoaderMsg::BuildErrors(build_diagnostics) => f.write_str("BuildErrors"),
    }
  }
}

pub struct AddEntryModuleMsg {
  pub chunk: Arc<EmittedChunk>,
  pub reference_id: ArcStr,
}
