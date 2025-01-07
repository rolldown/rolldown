use arcstr::ArcStr;
use bitflags::bitflags;
use oxc::{semantic::SymbolId, span::Span};
use oxc_index::IndexVec;
use rolldown_rstr::Rstr;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  side_effects::DeterminedSideEffects, types::source_mutation::BoxedSourceMutation, AstScopes,
  EcmaAstIdx, ExportsKind, ImportRecordIdx, LocalExport, ModuleDefFormat, ModuleId, NamedImport,
  ResolvedImportRecord, SourceMutation, StmtInfos, SymbolRef,
};

bitflags! {
    #[derive(Debug, Default)]
    pub struct EcmaViewMeta: u8 {
        const EVAL = 1;
        const INCLUDED = 1 << 1;
        const HAS_LAZY_EXPORT = 1 << 2;
        const HAS_STAR_EXPORT = 1 << 3;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThisExprReplaceKind {
  Undefined,
  Exports,
}

#[inline]
#[allow(clippy::implicit_hasher)]
pub fn generate_replace_this_expr_map(
  set: &FxHashSet<Span>,
  kind: ThisExprReplaceKind,
) -> FxHashMap<Span, ThisExprReplaceKind> {
  set.iter().map(|span| (*span, kind)).collect()
}

impl EcmaViewMeta {
  #[inline]
  pub fn has_eval(&self) -> bool {
    self.contains(Self::EVAL)
  }
  #[inline]
  pub fn is_included(&self) -> bool {
    self.contains(Self::INCLUDED)
  }
  #[inline]
  pub fn has_lazy_export(&self) -> bool {
    self.contains(Self::HAS_LAZY_EXPORT)
  }
  #[inline]
  pub fn has_star_export(&self) -> bool {
    self.contains(Self::HAS_STAR_EXPORT)
  }

  #[inline]
  pub fn set_eval(&mut self, value: bool) {
    if value {
      self.insert(Self::EVAL);
    } else {
      self.remove(Self::EVAL);
    }
  }
  #[inline]
  pub fn set_included(&mut self, value: bool) {
    if value {
      self.insert(Self::INCLUDED);
    } else {
      self.remove(Self::INCLUDED);
    }
  }
  #[inline]
  pub fn set_has_lazy_export(&mut self, value: bool) {
    if value {
      self.insert(Self::HAS_LAZY_EXPORT);
    } else {
      self.remove(Self::HAS_LAZY_EXPORT);
    }
  }
  #[inline]
  pub fn set_has_star_exports(&mut self, value: bool) {
    if value {
      self.insert(Self::HAS_STAR_EXPORT);
    } else {
      self.remove(Self::HAS_STAR_EXPORT);
    }
  }
}

#[derive(Debug)]
pub struct EcmaView {
  pub source: ArcStr,
  pub ecma_ast_idx: Option<EcmaAstIdx>,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub scope: AstScopes,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  // the ids of all modules that statically import this module
  pub importers: FxIndexSet<ModuleId>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: FxIndexSet<ModuleId>,
  // the module ids statically imported by this module
  pub imported_ids: FxIndexSet<ModuleId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
  pub side_effects: DeterminedSideEffects,
  pub ast_usage: EcmaModuleAstUsage,
  pub self_referenced_class_decl_symbol_ids: FxHashSet<SymbolId>,
  // the range of hashbang in source
  pub hashbang_range: Option<Span>,
  pub meta: EcmaViewMeta,
  pub mutations: Vec<BoxedSourceMutation>,
  /// `Span` of `new URL('path', import.meta.url)` -> `ImportRecordIdx`
  pub new_url_references: FxHashMap<Span, ImportRecordIdx>,
  pub this_expr_replace_map: FxHashMap<Span, ThisExprReplaceKind>,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct EcmaModuleAstUsage: u8 {
        const ModuleRef = 1;
        const ExportsRef = 1 << 1;
        const EsModuleFlag = 1 << 2;
        const ModuleOrExports = Self::ModuleRef.bits() | Self::ExportsRef.bits();
    }
}

#[derive(Debug, Default)]
pub struct ImportMetaRolldownAssetReplacer {
  pub asset_filename: ArcStr,
}

impl SourceMutation for ImportMetaRolldownAssetReplacer {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    magic_string
      .replace_all("import.meta.__ROLLDOWN_ASSET_FILENAME", format!("\"{}\"", self.asset_filename));
  }
}
