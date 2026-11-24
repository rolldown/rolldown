use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_error::BuildDiagnostic;
use runtime_task_result::RuntimeModuleTaskResult;
use task_result::{ExternalModuleTaskResult, NormalModuleTaskResult};

use crate::{EmittedChunk, ResolvedId};

pub mod runtime_module_brief;
pub mod runtime_task_result;
pub mod task_result;

pub enum ModuleLoaderMsg {
  NormalModuleDone(Box<NormalModuleTaskResult>),
  ExternalModuleDone(Box<ExternalModuleTaskResult>),
  RuntimeNormalModuleDone(Box<RuntimeModuleTaskResult>),
  FetchModule(Box<ResolvedId>),
  AddEntryModule(Box<AddEntryModuleMsg>),
  BuildErrors(Box<[BuildDiagnostic]>),
}

pub struct AddEntryModuleMsg {
  pub chunk: Arc<EmittedChunk>,
  pub reference_id: ArcStr,
}
