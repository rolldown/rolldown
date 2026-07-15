use crate::{ConstExportMeta, ImportAttribute, SourcemapChainElement};
use arcstr::ArcStr;
use bitflags::bitflags;
use oxc::{
  semantic::{NodeId, SymbolId},
  span::Span,
};
use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ExportsKind, HmrInfo, ImportRecordIdx, ImporterRecord, LocalExport, ModuleDefFormat, ModuleId,
  ModuleIdx, NamedImport, ResolvedImportRecord, SourceMutation, SymbolRef,
  side_effects::DeterminedSideEffects, types::source_mutation::ArcSourceMutation,
};

bitflags! {
    #[derive(Debug, Default, Clone, Copy)]
    pub struct EcmaViewMeta: u8 {
        const Eval = 1;
        const HasLazyExport = 1 << 1;
        const HasStarExport = 1 << 2;
        const SafelyTreeshakeCommonjs = 1 << 3;
        /// If a module has side effects or has top-level global variable access
        const ExecutionOrderSensitive = 1 << 4;
        /// If the module has top-level empty function, if any module has top level empty function, we need
        /// to apply cross module optimization.
        const TopExportedSideEffectsFreeFunction = 1 << 5;
        /// Module evaluation reads at least one imported binding.
        const TopLevelImportRead = 1 << 6;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThisExprReplaceKind {
  /// It depends on `context` set by the user. If it's unset, replace it with `undefined`.
  Context,
  Exports,
}

#[inline]
#[expect(clippy::implicit_hasher)]
pub fn generate_replace_this_expr_map(
  set: &FxHashSet<NodeId>,
  kind: ThisExprReplaceKind,
) -> FxHashMap<NodeId, ThisExprReplaceKind> {
  set.iter().map(|node_id| (*node_id, kind)).collect()
}

impl EcmaViewMeta {
  #[inline]
  pub fn has_eval(&self) -> bool {
    self.contains(Self::Eval)
  }
  #[inline]
  pub fn has_lazy_export(&self) -> bool {
    self.contains(Self::HasLazyExport)
  }
  #[inline]
  pub fn has_star_export(&self) -> bool {
    self.contains(Self::HasStarExport)
  }
}

/// Where a named export's value comes from: the module's own declaration, or a re-export of an
/// import.
///
/// This classification is the contract between the lazy-barrel loader and tree shaking's
/// body-demand gating, and the two sides MUST agree: the loader loads every plain import record
/// of a barrel as soon as one of its *own* exports is requested (`BarrelInfo::local` in
/// `take_needed_records`), and tree shaking includes the module's gated side-effect statements as
/// soon as one of its *own* exports is used (`compute_body_demand_keys`). If they classified an
/// export differently, a retained statement could reference an import record that was never
/// loaded — a free identifier at runtime (the #9806 bug family). Keeping the classification in
/// one place makes that agreement hold by construction.
pub enum ExportOrigin<'a> {
  /// Declared by the module itself: `export const a = ...`, `export function f() {}`, or a plain
  /// `export { local }` of a local binding.
  Own,
  /// Re-exports an import: `export { a } from './x'`, `export * as ns from './x'`, or
  /// `import { a } from './x'; export { a }`.
  ReExport(&'a NamedImport),
}

impl EcmaView {
  /// Classify a named export as the module's own declaration or a re-export of an import.
  /// See [`ExportOrigin`] for why this must stay the single source of truth.
  pub fn classify_export(&self, local_export: &LocalExport) -> ExportOrigin<'_> {
    match self.named_imports.get(&local_export.referenced) {
      Some(named_import) => ExportOrigin::ReExport(named_import),
      None => ExportOrigin::Own,
    }
  }
}

#[derive(Debug, Clone)]
pub struct EcmaView {
  pub dummy_record_set: FxHashSet<NodeId>,
  pub source: ArcStr,
  pub def_format: ModuleDefFormat,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<CompactStr, LocalExport>,
  pub import_records: IndexVec<ImportRecordIdx, ResolvedImportRecord>,
  /// Cross-pass AST-node side tables use post-semantic `NodeId`. See internal-docs/ast-mutation/implementation.md.
  ///
  /// The key is the `NodeId` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<NodeId, ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<SourcemapChainElement>,
  // the ids of all modules that statically import this module
  pub importers: FxIndexSet<ModuleId>,
  pub importers_idx: FxIndexSet<ModuleIdx>,
  // the ids of all modules that import this module via dynamic import()
  pub dynamic_importers: FxIndexSet<ModuleId>,
  // the idx of all modules that import this module via dynamic import()
  pub dynamic_importers_idx: FxIndexSet<ModuleIdx>,
  // the module ids statically imported by this module
  pub imported_ids: FxIndexSet<ModuleId>,
  // the module ids imported by this module via dynamic import()
  pub dynamically_imported_ids: FxIndexSet<ModuleId>,
  pub side_effects: DeterminedSideEffects,
  pub ast_usage: EcmaModuleAstUsage,
  pub self_referenced_class_decl_symbol_ids: FxHashSet<SymbolId>,
  // the range of hashbang in source
  pub hashbang_range: Option<Span>,
  pub directive_range: Vec<Span>,
  pub meta: EcmaViewMeta,
  pub mutations: Vec<ArcSourceMutation>,
  /// `NodeId` of `new URL('path', import.meta.url)` -> `ImportRecordIdx`
  pub new_url_references: FxHashMap<NodeId, ImportRecordIdx>,
  pub this_expr_replace_map: FxHashMap<NodeId, ThisExprReplaceKind>,

  pub hmr_hot_ref: Option<SymbolRef>,
  pub hmr_info: HmrInfo,
  pub constant_export_map: FxHashMap<SymbolId, ConstExportMeta>,
  /// Enum member constant values, keyed by enum name → member name → value.
  /// Used by the finalizer to inline `Direction.Up` style accesses across modules.
  /// Contains both const and regular enums.
  pub enum_member_value_map: FxHashMap<CompactStr, FxHashMap<CompactStr, ConstExportMeta>>,
  pub import_attribute_map: FxHashMap<ImportRecordIdx, ImportAttribute>,
  /// Use `Box` since it is rarely used also it could reduce the size of `EcmaView`, .
  pub json_module_none_self_reference_included_symbol: Option<Box<FxHashSet<SymbolRef>>>,
  /// Import record indices for `module.exports = require(...)` patterns.
  pub cjs_reexport_import_record_ids: Vec<ImportRecordIdx>,
}

impl EcmaView {
  /// Re-derives the importer sets from this module's `ImporterRecord`s.
  pub fn rebuild_importer_sets(&mut self, records: &[ImporterRecord]) {
    self.importers.clear();
    self.importers_idx.clear();
    self.dynamic_importers.clear();
    self.dynamic_importers_idx.clear();
    for record in records {
      if record.kind.is_static() {
        self.importers.insert(record.importer_path.clone());
        self.importers_idx.insert(record.importer_idx);
      } else {
        self.dynamic_importers.insert(record.importer_path.clone());
        // Only real `import()` edges join the walkable dynamic-importer set; `HotAccept`
        // and other non-static records are not import edges. Lazy-compilation proxy
        // importers (`?rolldown-lazy=1`) are an internal artifact — excluding them keeps
        // the server's superset walk from bubbling through the proxy chain
        // (`foo -> proxy -> app`), which patch generation is not set up for. Lazy
        // dynamic-import HMR is a separate follow-up; until then the walk stops at the
        // lazy entry, and a tab whose own walk crosses the proxy chain reloads itself
        // via its missing-factory / no-boundary paths, exactly as before.
        if record.kind.is_dynamic() && !record.importer_path.as_str().contains("?rolldown-lazy=1") {
          self.dynamic_importers_idx.insert(record.importer_idx);
        }
      }
    }
  }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct EcmaModuleAstUsage: u16 {
        const ModuleRef = 1;
        const ExportsRef = 1 << 1;
        /// If the module has `Object.defineProperty(module.exports, "__esModule", { value: true })` or it's variant
        const EsModuleFlag = 1 << 2;
        const AllStaticExportPropertyAccess = 1 << 3;
        /// module.exports = require('mod');
        const IsCjsReexport = 1 << 4;
        const TopLevelAwait = 1 << 5;
        const HmrSelfAccept = 1 << 6;
        const UnknownExportsRead = 1 << 7;
        /// Top-level return statement (only valid in CommonJS)
        const TopLevelReturn = 1 << 8;
        const ModuleOrExports = Self::ModuleRef.bits() | Self::ExportsRef.bits();
    }
}

#[derive(Debug, Default)]
pub struct PrependRenderedImport {
  pub intro: String,
}

impl SourceMutation for PrependRenderedImport {
  fn apply(&self, magic_string: &mut string_wizard::MagicString<'_>) {
    magic_string.append_intro(self.intro.clone());
  }
}
