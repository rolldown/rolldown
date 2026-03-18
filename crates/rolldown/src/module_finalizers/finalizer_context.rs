use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConcatenateWrappedModuleKind, ConstExportMeta, ImportKind,
  ImportRecordIdx, IndexModules, ModuleIdx, ModuleType, NormalModule, PathsOutputOption,
  RenderedConcatenatedModuleParts, RuntimeModuleBrief, SharedFileEmitter, SymbolRef, SymbolRefDb,
  WrapKind,
};
use rolldown_utils::IndexBitSet;

pub type FinalizerMutableFields = (
  FxIndexMap<ImportRecordIdx, String>, // transferred_import_record
  RenderedConcatenatedModuleParts,     // rendered_concatenated_wrapped_module_parts
);

use oxc::ast_visit::VisitMut as _;
use oxc_index::IndexVec;
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstSnippet;
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
  /// Pre-resolved paths for external modules (always a `FxHashMap` variant).
  pub resolved_paths: Option<&'me PathsOutputOption>,
  pub transitive_wrapped_deps: Option<&'me IndexVec<ModuleIdx, IndexBitSet<ModuleIdx>>>,
}

fn compute_minimal_init_set(
  module: &NormalModule,
  linking_infos: &LinkingMetadataVec,
  transitive_wrapped_deps: Option<&IndexVec<ModuleIdx, IndexBitSet<ModuleIdx>>>,
) -> Option<FxHashSet<ModuleIdx>> {
  let transitive_wrapped_deps = transitive_wrapped_deps?;

  // Collect direct wrapped ESM deps (deduped)
  let mut direct_deps = FxHashSet::default();
  for rec in &module.ecma_view.import_records {
    if rec.kind != ImportKind::Import {
      continue;
    }
    let Some(resolved) = rec.resolved_module else {
      continue;
    };
    let info = &linking_infos[resolved];
    if info.wrap_kind() != WrapKind::Esm {
      continue;
    }
    if matches!(info.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::Inner) {
      continue;
    }
    direct_deps.insert(resolved);
  }

  // Transitive reduction: drop dep di if any other dep dj (still in the minimal set)
  // transitively reaches di. We must only consider retained deps as covering,
  // otherwise circular deps would eliminate all members.
  let deps_vec: Vec<ModuleIdx> = direct_deps.iter().copied().collect();
  let mut minimal = direct_deps;
  for &di in &deps_vec {
    if !minimal.contains(&di) {
      continue;
    }
    let is_covered = deps_vec
      .iter()
      .any(|&dj| di != dj && minimal.contains(&dj) && transitive_wrapped_deps[dj].has_bit(di));
    if is_covered {
      minimal.remove(&di);
    }
  }
  Some(minimal)
}

impl<'me> ScopeHoistingFinalizerContext<'me> {
  #[tracing::instrument(level = "trace", skip_all)]
  pub fn finalize_normal_module(
    self,
    ast: &'me mut EcmaAst,
    ast_scope: &'me AstScopes,
  ) -> FinalizerMutableFields {
    let minimal_init_set =
      compute_minimal_init_set(self.module, self.linking_infos, self.transitive_wrapped_deps);

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

      let mut finalizer = ScopeHoistingFinalizer {
        alloc,
        ctx: self,
        scope: ast_scope,
        snippet: AstSnippet::new(alloc),
        generated_init_esm_importee_ids: FxHashSet::default(),
        minimal_init_set,
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
