use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConstExportMeta, ImportRecordIdx, IndexModules, ModuleIdx,
  ModuleType, NormalModule, PathsOutputOption, RenderedConcatenatedModuleParts,
  RetainedExportSymbols, RuntimeModuleBrief, SharedFileEmitter, StmtInfos, SymbolRef, SymbolRefDb,
  UsedSymbolRefs, WrapKind,
};

pub type FinalizerMutableFields = (
  FxIndexMap<ImportRecordIdx, String>, // transferred_import_record
  RenderedConcatenatedModuleParts,     // rendered_concatenated_wrapped_module_parts
  Vec<BuildDiagnostic>,                // diagnostics
);

use oxc::ast::builder::AstBuilder;
use oxc::ast_visit::VisitMut as _;
use oxc::semantic::NodeId;
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{BuildDiagnostic, CausedPlugin};
use rolldown_plugin::HookResolveFileUrlOutput;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::{ScopeHoistingFinalizer, TraverseState},
  stages::{
    generate_stage::order_wrap_state::{EsmInitOrigin, EsmInitTarget, OrderWrapState},
    link_stage::SafelyMergeCjsNsInfo,
  },
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub idx: ModuleIdx,
  pub chunk: &'me Chunk,
  pub chunk_idx: ChunkIdx,
  pub module: &'me NormalModule,
  /// Statement-info table for the current module, threaded in from the
  /// link-stage side `IndexVec<ModuleIdx, StmtInfos>` (see `LinkStage.stmt_infos`).
  pub stmt_infos: &'me StmtInfos,
  /// The full per-module statement-info table. Wrapped-ESM init target resolution needs another
  /// module's statement inclusion to tell whether an eager forwarder discharges its own hops.
  pub index_stmt_infos: &'me crate::type_alias::IndexStmtInfos,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub order_wrap_state: &'me OrderWrapState,
  pub used_symbol_refs: &'me UsedSymbolRefs,
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
  /// Plugin-supplied replacements for `import.meta.ROLLDOWN_FILE_URL_*`, keyed by
  /// `(module, NodeId of the member expression)`. Empty when no plugin implements the
  /// `resolveFileUrl` hook. The code is unparsed; this is the only place it is parsed.
  pub resolved_file_urls: &'me FxHashMap<(ModuleIdx, NodeId), HookResolveFileUrlOutput>,
  /// True if any module in the bundle has enum member values to inline.
  /// Allows skipping enum inlining checks in the hot visitor path for enum-free bundles.
  pub has_enum_inlining: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ModuleWrapperMode {
  None,
  InteropCjs(SymbolRef),
  InteropEsm(EsmInitTarget),
  ExecutionOrder(EsmInitTarget),
}

impl<'me> ScopeHoistingFinalizerContext<'me> {
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

    let Some(target) = self.order_wrap_state.esm_init_target(self.idx, self.linking_info) else {
      return ModuleWrapperMode::None;
    };
    let declaration_is_included = match target.origin {
      EsmInitOrigin::Interop => legacy_wrapper_is_included,
      EsmInitOrigin::ExecutionOrder => {
        self.order_wrap_state.order_wrapper_chunk(self.idx) == Some(self.chunk_idx)
      }
    };
    if !declaration_is_included {
      return ModuleWrapperMode::None;
    }

    match target.origin {
      EsmInitOrigin::Interop => ModuleWrapperMode::InteropEsm(target),
      EsmInitOrigin::ExecutionOrder => ModuleWrapperMode::ExecutionOrder(target),
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
        ast_builder: AstBuilder::new(alloc),
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
        missing_file_reference_ids: FxIndexMap::default(),
        resolve_file_url_errors: Vec::new(),
        surviving_import_meta_spans: FxIndexMap::default(),
      };
      finalizer.visit_program(oxc_program);

      let mut diagnostics = {
        let module = finalizer.ctx.module;
        finalizer
          .surviving_import_meta_spans
          .iter()
          .map(|(span, kind)| {
            BuildDiagnostic::empty_import_meta(
              module.id.to_string(),
              module.ecma_view.source.clone(),
              *span,
              finalizer.ctx.options.format.as_str().into(),
              *kind,
            )
            .with_severity_warning()
          })
          .collect::<Vec<_>>()
      };

      let missing_file_reference_ids = finalizer.missing_file_reference_ids;
      if !missing_file_reference_ids.is_empty() {
        let module = finalizer.ctx.module;
        diagnostics.extend(missing_file_reference_ids.into_iter().map(|(reference_id, span)| {
          BuildDiagnostic::file_not_found(
            reference_id.as_str(),
            module.id.to_string(),
            module.ecma_view.source.clone(),
            span,
          )
        }));
      }

      let mut resolve_file_url_errors = finalizer.resolve_file_url_errors;
      if !resolve_file_url_errors.is_empty() {
        // Dedup because a failed rewrite leaves the `import.meta.*` node in place, and the
        // visitor reaches the same node more than once, recording the failure each time.
        resolve_file_url_errors.sort_unstable();
        resolve_file_url_errors.dedup();
        // Attribute each failure to its plugin, so the user sees `[plugin foo] ...`
        diagnostics.extend(resolve_file_url_errors.into_iter().map(|(plugin_name, message)| {
          BuildDiagnostic::plugin_error(CausedPlugin::new(plugin_name), anyhow::anyhow!(message))
        }));
      }

      (
        finalizer.transferred_import_record,
        finalizer.rendered_concatenated_wrapped_module_parts,
        diagnostics,
      )
    })
  }
}
