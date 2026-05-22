use crate::{
  ImportRecordIdx, Module, ModuleId, ModuleIdx, RawImportRecord, ResolvedId, StmtInfos,
  SymbolRefDbForModule, dynamic_import_usage::DynamicImportExportsUsage,
  side_effects::DeterminedSideEffects, types::lazy_barrel::BarrelInfo,
};
use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildDiagnostic;
use rustc_hash::FxHashMap;

pub struct NormalModuleTaskResult {
  pub module: Module,
  pub ecma_related: EcmaRelated,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub warnings: Vec<BuildDiagnostic>,
  pub barrel_info: Option<BarrelInfo>,
  /// The span of the first top-level `await` keyword, if any. Collected by
  /// the module loader into a centralized map rather than stored per-module
  /// on `EcmaView`, since top-level await is rare.
  pub tla_keyword_span: Option<Span>,
}

pub struct ExternalModuleTaskResult {
  pub idx: ModuleIdx,
  pub id: ModuleId,
  pub name: ArcStr,
  pub identifier_name: ArcStr,
  pub side_effects: DeterminedSideEffects,
  pub need_renormalize_render_path: bool,
}

pub struct EcmaRelated {
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
  /// Whether JSX syntax is preserved for this module, determined per-module
  /// during transformation based on the resolved tsconfig.
  pub preserve_jsx: bool,
  /// Per-module statement-info table. Held alongside `EcmaView` rather than on
  /// it so the link stage can collect them into a side `IndexVec` without
  /// `mem::replace` and so reads/writes during link/generate can split-borrow
  /// from `metas`.
  pub stmt_infos: StmtInfos,
}
