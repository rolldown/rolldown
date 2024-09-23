use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use crate::css::css_view::CssView;
use crate::{
  DebugStmtInfoForTreeShaking, ExportsKind, ImportRecordIdx, ModuleId, ModuleIdx, ModuleInfo,
  StmtInfo,
};
use crate::{EcmaAstIdx, EcmaView, IndexModules, Interop, Module, ModuleType};

use rolldown_rstr::Rstr;
use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub is_user_defined_entry: bool,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: String,
  // Pretty resource id for debug
  pub debug_id: String,
  pub repr_name: String,
  pub module_type: ModuleType,
  pub ecma_view: EcmaView,
  pub css_view: Option<CssView>,
}

impl NormalModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.ecma_view.star_exports.iter().map(|rec_id| {
      let rec = &self.ecma_view.import_records[*rec_id];
      rec.resolved_module
    })
  }

  pub fn to_debug_normal_module_for_tree_shaking(&self) -> DebugNormalModuleForTreeShaking {
    DebugNormalModuleForTreeShaking {
      id: self.repr_name.to_string(),
      is_included: self.ecma_view.is_included,
      stmt_infos: self
        .ecma_view
        .stmt_infos
        .iter()
        .map(StmtInfo::to_debug_stmt_info_for_tree_shaking)
        .collect(),
    }
  }

  pub fn to_module_info(&self) -> ModuleInfo {
    ModuleInfo {
      code: Some(self.ecma_view.source.clone()),
      id: self.id.clone(),
      is_entry: self.is_user_defined_entry,
      importers: {
        let mut value = self.ecma_view.importers.clone();
        value.sort_unstable();
        value
      },
      dynamic_importers: {
        let mut value = self.ecma_view.dynamic_importers.clone();
        value.sort_unstable();
        value
      },
      imported_ids: self.ecma_view.imported_ids.clone(),
      dynamically_imported_ids: self.ecma_view.dynamically_imported_ids.clone(),
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
      .filter_map(|id| modules[id].as_normal())
      .for_each(|module| module.get_exported_names(export_star_set, modules, false, ret));
    if include_default {
      ret.extend(self.ecma_view.named_exports.keys());
    } else {
      ret.extend(self.ecma_view.named_exports.keys().filter(|name| name.as_str() != "default"));
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
    self.ecma_view.ecma_ast_idx.expect("ecma_ast_idx should be set in this stage")
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexModules,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.ecma_view.star_exports.iter().filter_map(move |rec_id| {
      let rec = &self.ecma_view.import_records[*rec_id];
      match modules[rec.resolved_module] {
        Module::External(_) => Some(*rec_id),
        Module::Normal(_) => None,
      }
    })
  }

  pub fn interop(&self) -> Option<Interop> {
    if matches!(self.ecma_view.exports_kind, ExportsKind::CommonJs) {
      if self.ecma_view.def_format.is_esm() {
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

impl Deref for NormalModule {
  type Target = EcmaView;

  fn deref(&self) -> &Self::Target {
    &self.ecma_view
  }
}

impl DerefMut for NormalModule {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ecma_view
  }
}
