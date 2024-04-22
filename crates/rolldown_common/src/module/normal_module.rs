use std::{fmt::Debug, sync::Arc};

use crate::{
  types::ast_scope::AstScope, DebugStmtInfoForTreeShaking, ExportsKind, ImportRecord,
  ImportRecordId, LocalExport, ModuleId, ModuleInfo, ModuleType, NamedImport, NormalModuleId,
  ResourceId, StmtInfo, StmtInfos, SymbolRef,
};
use index_vec::IndexVec;
use oxc::span::Span;
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub source: Arc<str>,
  pub id: NormalModuleId,
  pub is_user_defined_entry: bool,
  pub resource_id: ResourceId,
  pub pretty_path: String,
  /// Representative name of `FilePath`, which is created by `FilePath#representative_name` belong to `resource_id`
  pub repr_name: String,
  pub module_type: ModuleType,
  pub namespace_symbol: SymbolRef,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordId>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  pub exports_kind: ExportsKind,
  pub scope: AstScope,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  pub is_included: bool,
  // The runtime module and module which path starts with `\0` shouldn't generate sourcemap. Ref see https://github.com/rollup/rollup/blob/master/src/Module.ts#L279.
  pub is_virtual: bool,
}

impl NormalModule {
  pub fn star_export_modules(&self) -> impl Iterator<Item = ModuleId> + '_ {
    self.star_exports.iter().map(|rec_id| {
      let rec = &self.import_records[*rec_id];
      rec.resolved_module
    })
  }

  pub fn to_debug_normal_module_for_tree_shaking(&self) -> DebugNormalModuleForTreeShaking {
    DebugNormalModuleForTreeShaking {
      id: self.repr_name.to_string(),
      is_included: self.is_included,
      stmt_infos: self
        .stmt_infos
        .iter()
        .map(StmtInfo::to_debug_stmt_info_for_tree_shaking)
        .collect(),
    }
  }

  pub fn to_module_info(&self) -> ModuleInfo {
    ModuleInfo { code: Some(Arc::clone(&self.source)), id: self.resource_id.expect_file().clone() }
  }
}

#[derive(Debug)]
pub struct DebugNormalModuleForTreeShaking {
  pub id: String,
  pub is_included: bool,
  pub stmt_infos: Vec<DebugStmtInfoForTreeShaking>,
}
