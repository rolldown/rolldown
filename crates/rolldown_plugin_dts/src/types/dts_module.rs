use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  ImportRecordIdx, ImportRecordMeta, LocalExport, ModuleId, ModuleIdx, NamedImport,
  RawImportRecord, StmtInfos, SymbolRef,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_rstr::Rstr;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct DtsModule {
  pub module_index: ModuleIdx,
  pub module_id: ModuleId,
  pub stable_id: ArcStr,
  pub dts_ast: EcmaAst,
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub default_export_ref: SymbolRef,
  pub namespace_object_ref: SymbolRef,
  pub has_star_exports: bool,

  pub import_record_to_module_id: IndexVec<ImportRecordIdx, ModuleId>,
  pub import_record_to_module_idx: IndexVec<ImportRecordIdx, ModuleIdx>,
}

impl DtsModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    if self.has_star_exports {
      itertools::Either::Left(
        self
          .import_records
          .iter_enumerated()
          .filter(|(_, rec)| rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR))
          .map(|(rec_id, _)| self.import_record_to_module_idx[rec_id]),
      )
    } else {
      itertools::Either::Right(std::iter::empty())
    }
  }
}
