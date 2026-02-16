use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConstExportMeta, ImportRecordIdx, IndexModules, ModuleIdx,
  ModuleType, NormalModule, RenderedConcatenatedModuleParts, RuntimeModuleBrief, SharedFileEmitter,
  SymbolRef, SymbolRefDb,
};

pub type FinalizerMutableFields = (
  FxIndexMap<ImportRecordIdx, String>, // transferred_import_record
  RenderedConcatenatedModuleParts,     // rendered_concatenated_wrapped_module_parts
);

use oxc::ast_visit::VisitMut as _;
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_utils::IndexBitSet;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::{ScopeHoistingFinalizer, TraverseState},
  stages::link_stage::SafelyMergeCjsNsInfo,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub idx: ModuleIdx,
  pub chunk: &'me Chunk,
  pub chunk_idx: ChunkIdx,
  pub module: &'me NormalModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub file_emitter: &'me SharedFileEmitter,
  pub constant_value_map: &'me FxHashMap<SymbolRef, ConstExportMeta>,
  pub side_effect_free_function_symbols: &'me FxHashSet<SymbolRef>,
  pub safely_merge_cjs_ns_map: &'me FxHashMap<ModuleIdx, SafelyMergeCjsNsInfo>,
  pub used_symbol_refs: &'me FxHashSet<SymbolRef>,
}

impl<'me> ScopeHoistingFinalizerContext<'me> {
  #[tracing::instrument(level = "trace", skip_all)]
  pub fn finalize_normal_module(
    self,
    ast: &'me mut EcmaAst,
    ast_scope: &'me AstScopes,
  ) -> FinalizerMutableFields {
    ast.program.with_mut(move |fields| {
      let (oxc_program, alloc) = (fields.program, fields.allocator);

      let module_namespace_included =
        self.used_symbol_refs.contains(&self.module.namespace_object_ref);

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

      let modules_len = self.modules.len();
      let mut finalizer = ScopeHoistingFinalizer {
        alloc,
        ctx: self,
        scope: ast_scope,
        snippet: AstSnippet::new(alloc),
        generated_init_esm_importee_ids: IndexBitSet::new(modules_len),
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
