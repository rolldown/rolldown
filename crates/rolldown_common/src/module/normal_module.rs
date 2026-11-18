use std::fmt::Debug;

use crate::{
  EcmaModuleAstUsage, ExportsKind, ImportRecordIdx, ImportRecordMeta, ModuleId, ModuleIdx,
  ModuleInfo, RawImportRecord, ResolvedId, StableModuleId,
};
use crate::{EcmaView, IndexModules, Interop, Module, ModuleType};
use std::ops::{Deref, DerefMut};

use itertools::Itertools;
use oxc_index::IndexVec;
use oxc_str::CompactStr;

#[derive(Debug, Clone)]
pub struct NormalModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: StableModuleId,
  // Pretty resource id for debug. It can go in a `//` comment; see `ResolvedId::debug_id`.
  pub debug_id: String,
  pub repr_name: String,
  pub module_type: ModuleType,
  pub ecma_view: EcmaView,
  pub originative_resolved_id: ResolvedId,
}

impl NormalModule {
  pub fn star_export_records(&self) -> impl Iterator<Item = (ImportRecordIdx, ModuleIdx)> + '_ {
    self.ecma_view.import_records.iter_enumerated().filter_map(|(rec_idx, rec)| {
      rec
        .meta
        .contains(ImportRecordMeta::IsExportStar)
        .then(|| rec.resolved_module.map(|module_idx| (rec_idx, module_idx)))
        .flatten()
    })
  }

  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.star_export_records().map(|(_, module_idx)| module_idx)
  }

  pub fn has_star_export(&self) -> bool {
    self.ecma_view.meta.has_star_export()
  }

  pub fn to_module_info(
    &self,
    raw_import_records: Option<&IndexVec<ImportRecordIdx, RawImportRecord>>,
    is_entry: bool,
  ) -> ModuleInfo {
    ModuleInfo {
      code: Some(self.ecma_view.source.clone()),
      id: self.id.clone(),
      is_entry,
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
      // https://github.com/rollup/rollup/blob/7a8ac460c62b0406a749e367dbd0b74973282449/src/Module.ts#L331
      exports: {
        let mut exports = self.ecma_view.named_exports.keys().cloned().collect_vec();
        if let Some(e) = raw_import_records {
          exports.extend(
            e.iter()
              .filter(|&rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
              .map(|_| CompactStr::new("*")),
          );
        } else {
          exports.extend(
            self
              .ecma_view
              .import_records
              .iter()
              .filter(|&rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
              .map(|_| CompactStr::new("*")),
          );
        }
        exports
      },
      input_format: self.ecma_view.exports_kind,
    }
  }

  // The runtime module and module which path starts with `\0` shouldn't generate sourcemap. Ref see https://github.com/rollup/rollup/blob/master/src/Module.ts#L279.
  pub fn is_virtual(&self) -> bool {
    self.id.starts_with('\0')
  }

  pub fn star_exports_from_external_modules<'me>(
    &'me self,
    modules: &'me IndexModules,
  ) -> impl Iterator<Item = ImportRecordIdx> + 'me {
    self.ecma_view.import_records.iter_enumerated().filter_map(move |(rec_id, rec)| {
      if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
        return None;
      }
      match modules[rec.resolved_module?] {
        Module::External(_) => Some(rec_id),
        Module::Normal(_) => None,
      }
    })
  }

  // If the module is an ESM module that follows the Node.js ESM spec, such as
  // - extension is `.mjs`
  // - `package.json` has `"type": "module"`
  // , we need to consider to stimulate the Node.js ESM behavior for maximum compatibility.
  pub fn interop(&self, importee: &NormalModule) -> Option<Interop> {
    if matches!(importee.ecma_view.exports_kind, ExportsKind::CommonJs) {
      if self.ecma_view.def_format.is_esm() { Some(Interop::Node) } else { Some(Interop::Babel) }
    } else {
      None
    }
  }

  // If the module is an ESM module that follows the Node.js ESM spec, such as
  // - extension is `.mjs`
  // - `package.json` has `"type": "module"`
  // , we need to consider to stimulate the Node.js ESM behavior for maximum compatibility.
  #[inline]
  pub fn should_consider_node_esm_spec_for_static_import(&self) -> bool {
    self.ecma_view.def_format.is_esm()
  }

  #[inline]
  pub fn should_consider_node_esm_spec_for_dynamic_import(&self) -> bool {
    // Dynamic imports in cjs must be written targeting node platform.
    // So we always consider Node.js ESM spec for dynamic imports in cjs modules, even if modules aren't explicitly marked as cjs.
    self.ecma_view.def_format.is_esm() || self.ecma_view.def_format.is_commonjs()
  }

  #[expect(clippy::cast_precision_loss)]
  pub fn size(&self) -> f64 {
    self.ecma_view.source.len() as f64
  }

  pub fn is_hmr_self_accepting_module(&self) -> bool {
    self.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept)
  }

  pub fn can_accept_hmr_dependency_for(&self, module_id: &ModuleId) -> bool {
    self.hmr_info.deps.contains(module_id)
  }

  pub fn is_hmr_partially_accepting_module(&self) -> bool {
    self.ast_usage.contains(EcmaModuleAstUsage::HmrAcceptExports)
  }
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
