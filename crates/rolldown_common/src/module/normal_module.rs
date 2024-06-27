use std::{fmt::Debug, sync::Arc};

use crate::side_effects::DeterminedSideEffects;
use crate::ModuleType;
use crate::{
  types::ast_scopes::AstScopes, DebugStmtInfoForTreeShaking, ExportsKind, ImportRecord,
  ImportRecordId, LocalExport, ModuleDefFormat, ModuleId, ModuleInfo, NamedImport, NormalModuleId,
  ResourceId, StmtInfo, StmtInfos, SymbolRef,
};
use oxc::index::IndexVec;
use oxc::span::Span;
use rolldown_rstr::Rstr;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub source: Arc<str>,
  pub id: NormalModuleId,
  pub is_user_defined_entry: bool,
  pub resource_id: ResourceId,
  /// `stable_resource_id` is calculated based on `resource_id` to be stable across machine and os.
  pub stable_resource_id: String,
  // Pretty resource id for debug
  pub debug_resource_id: String,
  /// Representative name of `FilePath`, which is created by `FilePath#representative_name` belong to `resource_id`
  pub repr_name: String,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
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
  pub scope: AstScopes,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  pub is_included: bool,
  // the ids of all modules that statically import this module
  pub importers: Vec<ResourceId>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: Vec<ResourceId>,
  // the module ids statically imported by this module
  pub imported_ids: Vec<ResourceId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: Vec<ResourceId>,
  pub side_effects: DeterminedSideEffects,
  pub module_type: ModuleType,
}

impl NormalModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleId> + '_ {
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
    ModuleInfo {
      code: Some(Arc::clone(&self.source)),
      id: self.resource_id.clone(),
      is_entry: self.is_user_defined_entry,
      importers: {
        let mut value = self.importers.clone();
        value.sort_unstable();
        value
      },
      dynamic_importers: {
        let mut value = self.dynamic_importers.clone();
        value.sort_unstable();
        value
      },
      imported_ids: self.imported_ids.clone(),
      dynamically_imported_ids: self.dynamically_imported_ids.clone(),
    }
  }

  // The runtime module and module which path starts with `\0` shouldn't generate sourcemap. Ref see https://github.com/rollup/rollup/blob/master/src/Module.ts#L279.
  pub fn is_virtual(&self) -> bool {
    self.resource_id.starts_with('\0') || self.resource_id.starts_with("rolldown:")
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    export_star_set: &mut FxHashSet<NormalModuleId>,
    modules: &'modules IndexVec<NormalModuleId, NormalModule>,
    include_default: bool,
    ret: &mut FxHashSet<&'modules Rstr>,
  ) {
    if export_star_set.contains(&self.id) {
      return;
    }

    export_star_set.insert(self.id);

    self
      .star_export_module_ids()
      .filter_map(ModuleId::as_normal)
      .for_each(|id| modules[id].get_exported_names(export_star_set, modules, false, ret));
    if include_default {
      ret.extend(self.named_exports.keys());
    } else {
      ret.extend(self.named_exports.keys().filter(|name| name.as_str() != "default"));
    }
  }

  // // https://tc39.es/ecma262/#sec-getexportednames
  // pub fn get_exported_names<'module>(
  //   &'module self,
  //   export_star_set: &mut FxHashSet<NormalModuleId>,
  //   ret: &mut FxHashSet<&'module Rstr>,
  //   modules: &'module IndexVec<NormalModuleId, NormalModule>,
  // ) {
  //   if export_star_set.contains(&self.id) {
  //     // noop
  //   } else {
  //     export_star_set.insert(self.id);
  //     ret.extend(self.named_exports.keys().filter(|name| name.as_str() != "default"));
  //     self.star_export_modules().for_each(|importee_id| match importee_id {
  //       ModuleId::Normal(importee_id) => {
  //         modules[importee_id].get_exported_names(export_star_set, ret, modules)
  //       }
  //       ModuleId::External(_) => {}
  //     });
  //   }
  // }
}

#[derive(Debug)]
pub struct DebugNormalModuleForTreeShaking {
  pub id: String,
  pub is_included: bool,
  pub stmt_infos: Vec<DebugStmtInfoForTreeShaking>,
}
