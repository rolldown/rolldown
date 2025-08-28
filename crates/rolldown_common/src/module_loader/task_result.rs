use crate::{
  ImportRecordIdx, Module, ModuleIdx, RawImportRecord, ResolvedId, SymbolRefDbForModule,
  dynamic_import_usage::DynamicImportExportsUsage, side_effects::DeterminedSideEffects,
};
use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildDiagnostic;
use rustc_hash::FxHashMap;

pub struct NormalModuleTaskResult {
  pub module: Module,
  pub ecma_related: EcmaRelated,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildDiagnostic>,
}

pub struct ExternalModuleTaskResult {
  pub idx: ModuleIdx,
  pub id: ArcStr,
  pub name: ArcStr,
  pub identifier_name: ArcStr,
  pub side_effects: DeterminedSideEffects,
  pub need_renormalize_render_path: bool,
}

pub struct EcmaRelated {
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}
