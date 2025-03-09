use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;

use crate::{ImportRecordIdx, NormalModule, RawImportRecord, ResolvedId, SymbolRefDbForModule};

use super::runtime_module_brief::RuntimeModuleBrief;

pub struct RuntimeModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub local_symbol_ref_db: SymbolRefDbForModule,
  pub ast: EcmaAst,
  pub module: NormalModule,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
}
