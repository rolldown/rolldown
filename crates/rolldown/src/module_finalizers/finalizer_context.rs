use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConstExportMeta, ImportRecordIdx, IndexModules, ModuleIdx,
  NormalModule, RenderedConcatenatedModuleParts, RuntimeModuleBrief, SharedFileEmitter, SymbolRef,
  SymbolRefDb,
};

use oxc::{allocator::TakeIn as _, ast_visit::VisitMut as _, span::CompactStr};
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::ScopeHoistingFinalizer,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
};

pub struct ScopeHoistingFinalizerContext<'me> {
  pub id: ModuleIdx,
  pub chunk: &'me Chunk,
  pub chunk_id: ChunkIdx,
  pub module: &'me NormalModule,
  pub modules: &'me IndexModules,
  pub linking_info: &'me LinkingMetadata,
  pub linking_infos: &'me LinkingMetadataVec,
  pub symbol_db: &'me SymbolRefDb,
  pub runtime: &'me RuntimeModuleBrief,
  pub chunk_graph: &'me ChunkGraph,
  pub options: &'me SharedOptions,
  pub cur_stmt_index: usize,
  pub keep_name_statement_to_insert: Vec<(usize, CompactStr, CompactStr)>,
  pub file_emitter: &'me SharedFileEmitter,
  pub constant_value_map: &'me FxHashMap<SymbolRef, ConstExportMeta>,
  pub needs_hosted_top_level_binding: bool,
  pub module_namespace_included: bool,
  pub transferred_import_record: FxIndexMap<ImportRecordIdx, String>,
  pub rendered_concatenated_wrapped_module_parts: RenderedConcatenatedModuleParts,
}

impl<'me> ScopeHoistingFinalizerContext<'me> {
  #[tracing::instrument(level = "trace", skip_all)]
  pub fn finalize_normal_module(self, ast: &'me mut EcmaAst, ast_scope: &'me AstScopes) -> Self {
    ast.program.with_mut(move |fields| {
      let (oxc_program, alloc) = (fields.program, fields.allocator);
      let mut finalizer = ScopeHoistingFinalizer {
        alloc,
        ctx: self,
        scope: ast_scope,
        snippet: AstSnippet::new(alloc),
        comments: oxc_program.comments.take_in(alloc),
        generated_init_esm_importee_ids: FxHashSet::default(),
        scope_stack: vec![],
        top_level_var_bindings: FxIndexSet::default(),
        is_top_level: false,
      };
      finalizer.visit_program(oxc_program);
      oxc_program.comments = finalizer.comments.take_in(alloc);
      finalizer.ctx
    })
  }
}
