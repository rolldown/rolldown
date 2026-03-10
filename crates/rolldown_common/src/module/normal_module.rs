use std::collections::HashSet;
use std::{fmt::Debug, sync::Arc};

use crate::types::module_render_output::ModuleRenderOutput;
use crate::{
  DebugStmtInfoForTreeShaking, EcmaModuleAstUsage, ExportsKind, ImportRecordIdx, ImportRecordMeta,
  ModuleId, ModuleIdx, ModuleInfo, NormalizedBundlerOptions, RawImportRecord, ResolvedId,
  StableModuleId, StmtInfo, StmtInfoIdx, SymbolRef,
};
use crate::{EcmaView, IndexModules, Interop, Module, ModuleType};
use std::ops::{Deref, DerefMut};

use itertools::Itertools;
use oxc::span::CompactStr;
use oxc_index::IndexVec;
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintOptions};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::IndexBitSet;
use rustc_hash::FxHashSet;
use string_wizard::SourceMapOptions;

#[derive(Debug, Clone)]
pub struct NormalModule {
  pub exec_order: u32,
  pub idx: ModuleIdx,
  pub id: ModuleId,
  /// `stable_id` is calculated based on `id` to be stable across machine and os.
  pub stable_id: StableModuleId,
  // Pretty resource id for debug
  pub debug_id: String,
  pub repr_name: String,
  pub module_type: ModuleType,
  pub ecma_view: EcmaView,
  pub originative_resolved_id: ResolvedId,
}

impl NormalModule {
  pub fn star_export_module_ids(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    if self.has_star_export() {
      itertools::Either::Left(self.ecma_view.import_records.iter().filter_map(|rec| {
        if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
          return None;
        }
        rec.resolved_module
      }))
    } else {
      itertools::Either::Right(std::iter::empty())
    }
  }

  pub fn has_star_export(&self) -> bool {
    self.ecma_view.meta.has_star_export()
  }

  pub fn to_debug_normal_module_for_tree_shaking(
    &self,
    is_included: bool,
    stmt_info_included: &IndexBitSet<StmtInfoIdx>,
  ) -> DebugNormalModuleForTreeShaking {
    DebugNormalModuleForTreeShaking {
      id: self.repr_name.clone(),
      is_included,
      stmt_infos: self
        .ecma_view
        .stmt_infos
        .iter_enumerated()
        .map(|(idx, stmt)| {
          stmt.to_debug_stmt_info_for_tree_shaking(stmt_info_included.has_bit(idx))
        })
        .collect(),
    }
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
    self.id.starts_with('\0') || self.id.starts_with("rolldown:")
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    export_star_set: &mut FxHashSet<ModuleIdx>,
    modules: &'modules IndexModules,
    include_default: bool,
    ret: &mut FxHashSet<&'modules CompactStr>,
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

  pub fn render(
    &self,
    options: &NormalizedBundlerOptions,
    args: &ModuleRenderArgs,
    initial_indent: u32,
  ) -> ModuleRenderOutput {
    match args {
      ModuleRenderArgs::Ecma { ast } => {
        let enable_sourcemap = options.sourcemap.is_some() && !self.is_virtual();

        // Because oxc codegen sourcemap is last of sourcemap chain,
        // If here no extra sourcemap need remapping, we using it as final module sourcemap.
        // So here make sure using correct `source_name` and `source_content.
        let render_output = EcmaCompiler::print_with(
          ast,
          PrintOptions {
            sourcemap: enable_sourcemap,
            filename: self.id.to_string(),
            comments: options.comments.into(),
            initial_indent,
          },
        );
        if !self.ecma_view.mutations.is_empty() {
          let original_code: Arc<str> = render_output.code.into();
          let mut magic_string = string_wizard::MagicString::new(&*original_code);
          for mutation in &self.ecma_view.mutations {
            mutation.apply(&mut magic_string);
          }
          let code = magic_string.to_string();
          let mutated_map = magic_string.source_map(SourceMapOptions {
            source: Arc::clone(&original_code),
            ..Default::default()
          });
          let map =
            render_output.map.map(|original| collapse_sourcemaps(&[&original, &mutated_map]));
          return ModuleRenderOutput { code, map };
        }
        ModuleRenderOutput { code: render_output.code, map: render_output.map }
      }
    }
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

  pub fn is_pure_reexport_module(&self) -> bool {
    //  First check if all stmt_info are re-export, if yes return true
    if self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .all(|(_, stmt_info)| self.is_reexport_statement(stmt_info))
    {
      return true;
    }

    // Check if all stmt_info are plain import ,return false
    if self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .all(|(_, stmt_info)| self.is_plain_import_statement(stmt_info))
    {
      return false;
    }

    let is_any_reexport = self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .any(|(_, stmt_info)| self.is_reexport_statement(stmt_info));

    let is_any_plain_import = self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .any(|(_, stmt_info)| self.is_plain_import_statement(stmt_info));

    let is_export_only = self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .any(|(_, stmt_info)| self.is_export_only_statement(stmt_info));

    //Check if there are any statements that are neither re-exports nor plain imports with matched exports
    if !(is_any_reexport || is_any_plain_import || is_export_only) {
      return false;
    }
    // Extract symbols from is_export_only and is_plain_import_statement, compare them
    let export_only_symbols: HashSet<SymbolRef> = self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .filter(|(_, stmt_info)| self.is_export_only_statement(stmt_info))
      .flat_map(|(_, stmt_info)| &stmt_info.referenced_symbols)
      .map(|symbol| *symbol.symbol_ref())
      .collect();

    let plain_import_symbols: HashSet<SymbolRef> = self
      .stmt_infos
      .iter_enumerated_without_namespace_stmt()
      .filter(|(_, stmt_info)| self.is_plain_import_statement(stmt_info))
      .flat_map(|(_, stmt_info)| &stmt_info.declared_symbols)
      .map(|tagged_symbol| tagged_symbol.inner())
      .collect();

    // If all export-only symbols match plain import symbols, return true
    // Otherwise, return false
    export_only_symbols.iter().all(|symbol_ref| plain_import_symbols.contains(symbol_ref))
  }

  // Helper function to determine if a statement is a re-export statement
  fn is_reexport_statement(&self, stmt_info: &StmtInfo) -> bool {
    let export_str = stmt_info.stmt_str.as_ref();

    if export_str.is_none() {
      return false;
    }

    let export_str = export_str.unwrap().trim();

    if export_str.contains("import") {
      return false;
    }
    stmt_info.import_records.iter().any(|&record_idx| {
      self
        .named_imports
        .values()
        .any(|named_import| named_import.record_idx == record_idx && named_import.is_reexport)
    })
  }

  ///  Helper function to determine if a statement is a plain import statement
  /// (imports only, no re-export)
  fn is_plain_import_statement(&self, stmt_info: &StmtInfo) -> bool {
    let import_str = stmt_info.stmt_str.as_ref();

    if import_str.is_none() {
      return false;
    }

    let import_str = import_str.unwrap().trim();

    if !(import_str.starts_with("import") && import_str.contains("from")) {
      return false;
    }
    !stmt_info.import_records.is_empty()
      && !stmt_info.import_records.iter().any(|&record_idx| {
        self
          .named_imports
          .values()
          .any(|named_import| named_import.record_idx == record_idx && named_import.is_reexport)
      })
  }

  /// Helper function to determine if a statement is an export-only statement
  /// (export { A } but not export { A } from ...)
  fn is_export_only_statement(&self, stmt_info: &StmtInfo) -> bool {
    let export_str = stmt_info.stmt_str.as_ref();

    if export_str.is_none() {
      return false;
    }

    let export_str = export_str.unwrap().trim();

    if export_str.contains("from")
      || export_str.contains("default")
      || export_str.contains("import")
    {
      return false;
    }

    // Check if this statement references symbols that are also in named_imports
    stmt_info.referenced_symbols.iter().any(|symbol_or_member_ref| {
      match symbol_or_member_ref {
        crate::SymbolOrMemberExprRef::Symbol(symbol_ref) => {
          // Check if this symbol is imported in this module
          let is_containes = self.named_imports.contains_key(symbol_ref);

          if is_containes {
            let named_import = self.named_imports.get(symbol_ref).unwrap();
            let litera_specifier = named_import.imported.get_literal();

            if named_import.is_reexport || litera_specifier.is_none() {
              return false;
            }
            let literal_str = litera_specifier.unwrap();
            let local_export = self.named_exports.get(literal_str);
            return local_export.is_some()
              && !local_export.unwrap().came_from_commonjs
              && &local_export.unwrap().referenced == symbol_ref;
          }
          false
        }
        crate::SymbolOrMemberExprRef::MemberExpr(_) => false,
      }
    })
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

pub enum ModuleRenderArgs<'any> {
  Ecma { ast: &'any EcmaAst },
}
