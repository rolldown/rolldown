use oxc_index::IndexVec;
use rolldown_common::{
  ImportRecordIdx, LocalExport, ModuleId, ModuleIdx, NamedImport, RawImportRecord, StmtInfos,
  SymbolRef, SymbolRefDbForModule,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_rstr::Rstr;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct DtsModule {
  pub module_index: ModuleIdx,
  pub module_id: ModuleId,
  pub dts_ast: EcmaAst,
  pub symbol_ref_db: SymbolRefDbForModule,

  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub default_export_ref: SymbolRef,
  pub namespace_object_ref: SymbolRef,
}
