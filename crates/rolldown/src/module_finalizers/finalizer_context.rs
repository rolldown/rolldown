use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConstExportMeta, ImportRecordIdx, IndexModules, ModuleIdx,
  ModuleType, NormalModule, PathsOutputOption, RenderedConcatenatedModuleParts,
  RetainedExportSymbols, RuntimeModuleBrief, SharedFileEmitter, StmtInfos, SymbolRef, SymbolRefDb,
  WrapKind,
};

pub type FinalizerMutableFields = (
  FxIndexMap<ImportRecordIdx, String>, // transferred_import_record
  RenderedConcatenatedModuleParts,     // rendered_concatenated_wrapped_module_parts
);

use oxc::ast_visit::VisitMut as _;
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstFactory;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::{ScopeHoistingFinalizer, TraverseState},
  stages::link_stage::SafelyMergeCjsNsInfo,
  types::linking_metadata::{EsmInitTarget, LinkingMetadata, LinkingMetadataVec},
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub idx: ModuleIdx,
  pub chunk: &'me Chunk,
  pub chunk_idx: ChunkIdx,
  pub module: &'me NormalModule,
  /// Statement-info table for the current module, threaded in from the
  /// link-stage side `IndexVec<ModuleIdx, StmtInfos>` (see `LinkStage.stmt_infos`).
  pub stmt_infos: &'me StmtInfos,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub file_emitter: &'me SharedFileEmitter,
  pub constant_value_map: &'me FxHashMap<SymbolRef, ConstExportMeta>,
  pub safely_merge_cjs_ns_map: &'me FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub retained_export_symbols: &'me RetainedExportSymbols,
  /// Pre-resolved paths for external modules (always a `FxHashMap` variant).
  pub resolved_paths: Option<&'me PathsOutputOption>,
  /// True if any module in the bundle has enum member values to inline.
  /// Allows skipping enum inlining checks in the hot visitor path for enum-free bundles.
  pub has_enum_inlining: bool,
}

/// How the finalizer must wrap the current module, resolved once from its linking metadata.
#[derive(Clone, Copy, Debug)]
pub(super) enum ModuleWrapperMode {
  /// No wrapper declaration is emitted for this module.
  None,
  /// CJS interop wrapper (`__commonJS`); carries the wrapper symbol.
  InteropCjs(SymbolRef),
  /// ESM interop wrapper (`__esm`); carries the init target the call sites reference.
  InteropEsm(EsmInitTarget),
}

impl<'me> ScopeHoistingFinalizerContext<'me> {
  /// Classify how this module's wrapper is emitted. Replaces the finalizer's inline
  /// `wrap_kind() + wrapper-statement-included` checks with a single view so all wrapper and
  /// `init_*()` decisions read the same source of truth.
  pub(super) fn wrapper_mode(&self) -> ModuleWrapperMode {
    let legacy_wrapper_is_included = self
      .linking_info
      .wrapper_stmt_info
      .is_some_and(|stmt_idx| self.linking_info.stmt_info_included.has_bit(stmt_idx));

    if matches!(self.linking_info.wrap_kind(), WrapKind::Cjs) && legacy_wrapper_is_included {
      return ModuleWrapperMode::InteropCjs(
        self.linking_info.wrapper_ref.expect("included CJS wrapper should have a symbol"),
      );
    }

    match self.linking_info.esm_init_target() {
      Some(target) if legacy_wrapper_is_included => ModuleWrapperMode::InteropEsm(target),
      _ => ModuleWrapperMode::None,
    }
  }

  #[tracing::instrument(level = "trace", skip_all)]
  pub fn finalize_normal_module(
    self,
    ast: &'me mut EcmaAst,
    ast_scope: &'me AstScopes,
  ) -> FinalizerMutableFields {
    ast.program.with_mut(move |fields| {
      let (oxc_program, alloc) = (fields.program, fields.allocator);

      let module_namespace_included = self.linking_info.namespace_included;

      let need_inline_json_prop = matches!(self.module.module_type, ModuleType::Json)
        && !self.module.exports_kind.is_commonjs()
        && !module_namespace_included;

      let transferred_import_record = self
        .chunk
        .remove_map
        .get(&self.idx)
        .cloned()
        .map(|idxs| idxs.into_iter().map(|idx| (idx, String::new())).collect::<FxIndexMap<_, _>>())
        .unwrap_or_default();

      let mut finalizer = ScopeHoistingFinalizer {
        alloc,
        ctx: self,
        scope: ast_scope,
        ast_factory: AstFactory::new(alloc),
        generated_init_esm_importee_ids: FxHashSet::default(),
        scope_stack: vec![],
        top_level_var_bindings: FxIndexSet::default(),
        state: TraverseState::empty(),
        cur_stmt_index: 0,
        keep_name_statement_to_insert: vec![],
        needs_hosted_top_level_binding: false,
        module_namespace_included,
        transferred_import_record,
        rendered_concatenated_wrapped_module_parts: RenderedConcatenatedModuleParts::default(),
        json_module_inlined_prop: need_inline_json_prop.then(|| Box::new(FxHashMap::default())),
      };
      finalizer.visit_program(oxc_program);
      (finalizer.transferred_import_record, finalizer.rendered_concatenated_wrapped_module_parts)
    })
  }
}
