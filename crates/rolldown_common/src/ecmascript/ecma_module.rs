use std::fmt::Debug;

use crate::side_effects::DeterminedSideEffects;
use crate::{
  types::ast_scopes::AstScopes, DebugStmtInfoForTreeShaking, ExportsKind, ImportRecord,
  ImportRecordIdx, LocalExport, ModuleDefFormat, ModuleId, ModuleIdx, ModuleInfo, NamedImport,
  StmtInfo, StmtInfos, SymbolRef,
};
use crate::{EcmaAstIdx, IndexModules, Interop, Module, ModuleType};
use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::span::Span;
use rolldown_rstr::Rstr;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug)]
pub struct EcmaModule {
  pub exec_order: u32,
  pub source: ArcStr,
  pub idx: ModuleIdx,
  pub ecma_ast_idx: Option<EcmaAstIdx>,
  pub is_user_defined_entry: bool,
  pub has_eval: bool,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: String,
  // Pretty resource id for debug
  pub debug_id: String,
  pub repr_name: String,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, ImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub scope: AstScopes,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  pub is_included: bool,
  // the ids of all modules that statically import this module
  pub importers: Vec<ModuleId>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: Vec<ModuleId>,
  // the module ids statically imported by this module
  pub imported_ids: Vec<ModuleId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: Vec<ModuleId>,
  pub side_effects: DeterminedSideEffects,
  pub module_type: ModuleType,
}

impl EcmaModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
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
      code: Some(self.source.clone()),
      id: self.id.clone(),
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
    self.id.starts_with('\0') || self.id.starts_with("rolldown:")
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    export_star_set: &mut FxHashSet<ModuleIdx>,
    modules: &'modules IndexModules,
    include_default: bool,
    ret: &mut FxHashSet<&'modules Rstr>,
  ) {
    if export_star_set.contains(&self.idx) {
      return;
    }

    export_star_set.insert(self.idx);

    self
      .star_export_module_ids()
      .filter_map(|id| modules[id].as_ecma())
      .for_each(|module| module.get_exported_names(export_star_set, modules, false, ret));
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

  pub fn ecma_ast_idx(&self) -> EcmaAstIdx {
    self.ecma_ast_idx.expect("ecma_ast_idx should be set in this stage")
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexModules,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.star_exports.iter().filter_map(move |rec_id| {
      let rec = &self.import_records[*rec_id];
      match modules[rec.resolved_module] {
        Module::External(_) => Some(*rec_id),
        Module::Ecma(_) => None,
      }
    })
  }

  pub fn interop(&self) -> Option<Interop> {
    if matches!(self.exports_kind, ExportsKind::CommonJs) {
      if self.def_format.is_esm() {
        Some(Interop::Node)
      } else {
        Some(Interop::Babel)
      }
    } else {
      None
    }
  }
}

#[derive(Debug)]
pub struct DebugNormalModuleForTreeShaking {
  pub id: String,
  pub is_included: bool,
  pub stmt_infos: Vec<DebugStmtInfoForTreeShaking>,
}
