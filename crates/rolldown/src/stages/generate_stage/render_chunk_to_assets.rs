use std::{ops::Deref, pin::Pin, sync::Arc};

use arcstr::ArcStr;
use futures::future::try_join_all;
use oxc_index::{IndexVec, index_vec};
use oxc_str::CompactStr;
use rolldown_common::{
  Asset, ChunkIdx, EmittedChunkInfo, InstantiationKind, ModuleIdx, ModuleRenderOutput, Output,
  OutputAsset, OutputChunk, SharedFileEmitter, SymbolRef, UsedSymbolRefs,
};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult, Diagnostics};
use rolldown_utils::{
  index_vec_ext::IndexVecRefExt as _,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::ParallelIterator as _,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  BundleOutput,
  chunk_graph::ChunkGraph,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{AssetVec, IndexChunkToInstances, IndexInstantiatedChunks},
  types::{
    generator::{
      GenerateContext, GenerateOutput, Generator, MultiModuleChunkLookup, MultiModuleChunkRender,
    },
    scan_stage_cache::CachedNativeChunk,
  },
  utils::{
    augment_chunk_hash::augment_chunk_hash,
    chunk::{finalize_chunks::finalize_assets, render_chunk_exports::get_export_items},
    render_chunks::render_chunks,
  },
};

use super::{CarriedRender, GenerateStage};

type ChunkGeneratorFuture<'a> = Pin<
  Box<
    dyn Future<Output = Result<Result<GenerateOutput, BatchedBuildDiagnostic>, anyhow::Error>>
      + 'a
      + Send,
  >,
>;

impl GenerateStage<'_> {
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &ChunkGraph,
    rendered_modules: IndexVec<ModuleIdx, Option<ModuleRenderOutput>>,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &super::Sealed<super::FinalEsmInitMetadata>,
    chunk_decisions: &IndexVec<ChunkIdx, Option<[u128; 2]>>,
    reused_chunks: &FxHashSet<ChunkIdx>,
  ) -> BuildResult<BundleOutput> {
    // Move the mixed-severity accumulator out of `link_output` so it can be
    // passed by `&mut` to `instantiate_chunks` (which borrows `&self`, and so
    // cannot also borrow `self.link_output.diagnostics`). The split into
    // warnings vs. errors is deferred to the end of the function, where their
    // fates finally diverge (errors -> `Err`, warnings -> `BundleOutput`).
    let mut diagnostics = std::mem::take(&mut self.link_output.diagnostics);
    let inline_renders = std::mem::take(&mut self.inline_renders);
    let (mut skip_sourcemap_chunks, multi_module_chunk_bases) = self.prepare_chunk_sourcemap_cache(
      chunk_graph,
      &rendered_modules,
      order_state,
      reused_chunks,
    );
    let (mut instantiated_chunks, index_chunk_to_instances, multi_module_renders) = self
      .instantiate_chunks(
        chunk_graph,
        rendered_modules,
        &mut diagnostics,
        used_symbol_refs,
        order_state,
        final_esm_init_metadata,
        reused_chunks,
        &skip_sourcemap_chunks,
        &multi_module_chunk_bases,
        inline_renders,
      )
      .await?;
    if let Some(cache) = self.cache.as_deref_mut() {
      cache.current_rendered_sources = FxHashMap::default();
    }
    let multi_module_chunks =
      self.cache_multi_module_chunks(multi_module_renders, &mut skip_sourcemap_chunks);

    self.trace_action_package_graph_ready(chunk_graph, &instantiated_chunks);
    self.trace_action_module_rendered(&instantiated_chunks);

    let runtime_chunk_id = chunk_graph.module_to_chunk[self.link_output.runtime.id()];
    for ins_chunk in &instantiated_chunks {
      if Some(ins_chunk.originate_from) == runtime_chunk_id
        && let InstantiationKind::Ecma(ecma_meta) = &ins_chunk.kind
        && let Some(cache) = self.cache.as_deref_mut()
      {
        cache.runtime_rendered_chunk = Some(Arc::clone(&ecma_meta.rendered_chunk));
      }
    }
    let extra_runtime_chunk =
      self.cache.as_ref().and_then(|cache| cache.runtime_rendered_chunk.as_ref());

    diagnostics.extend(
      render_chunks(
        self.plugin_driver,
        &mut instantiated_chunks,
        self.options,
        &chunk_graph.chunk_table,
        &self.link_output.module_table,
        extra_runtime_chunk,
      )
      .await?,
    );

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let mangle_cache = self.minify_chunks(&mut instantiated_chunks, &skip_sourcemap_chunks)?;

    Self::post_banner_footer(&mut instantiated_chunks)?;

    let assets = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated_chunks,
      &index_chunk_to_instances,
      self.options.hash_characters,
      self.options,
      self.resolved_paths.as_ref(),
      Some(&multi_module_chunks),
      self.cache.as_deref_mut(),
    )
    .await?;

    // Set emitted chunk info for file emitter, it should be set before call generate_bundle hook
    set_emitted_chunk_filenames(&self.plugin_driver.file_emitter, &assets, chunk_graph);

    Self::trace_action_assets_ready(&assets);

    let mut output = Vec::with_capacity(assets.len() + reused_chunks.len() * 2);
    let mut newly_cached_chunks: FxHashMap<ChunkIdx, Arc<OutputChunk>> = FxHashMap::default();
    let mut newly_cached_maps: FxHashMap<ChunkIdx, Arc<OutputAsset>> = FxHashMap::default();
    let strip_cached_modules = self.cache.as_ref().is_some_and(|c| c.omit_reused_chunk_bytes)
      && crate::utils::native_records::enabled(self.options)
      && !trace_action_enabled!();
    for Asset { map, meta: rendered_chunk, content: code, filename, originate_from } in assets {
      match rendered_chunk {
        InstantiationKind::Ecma(ecma_meta) => {
          let code = code.try_into_string()?;
          let rendered_chunk = ecma_meta.rendered_chunk;
          let chunk_arc = Arc::new(OutputChunk {
            name: rendered_chunk.name.clone(),
            filename: filename.clone(),
            code,
            is_entry: rendered_chunk.is_entry,
            is_dynamic_entry: rendered_chunk.is_dynamic_entry,
            facade_module_id: rendered_chunk.facade_module_id.clone(),
            modules: if strip_cached_modules {
              FxHashMap::default().into()
            } else {
              rendered_chunk.modules.clone()
            },
            exports: rendered_chunk.exports.clone(),
            module_ids: rendered_chunk.module_ids.clone(),
            imports: ecma_meta.imports,
            dynamic_imports: ecma_meta.dynamic_imports,
            map,
            sourcemap_filename: ecma_meta.sourcemap_filename,
            preliminary_filename: ecma_meta.preliminary_filename.to_string(),
          });
          if let Some(chunk_idx) = originate_from
            && chunk_decisions.get(chunk_idx).and_then(|d| *d).is_some()
          {
            newly_cached_chunks.insert(chunk_idx, Arc::clone(&chunk_arc));
          }
          output.push(Output::Chunk(chunk_arc));
        }
        InstantiationKind::Sourcemap(sourcemap_meta) => {
          let asset_arc = Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: sourcemap_meta.original_file_names,
            names: sourcemap_meta.names,
          });
          if let Some(chunk_idx) = originate_from
            && chunk_decisions.get(chunk_idx).and_then(|d| *d).is_some()
          {
            newly_cached_maps.insert(chunk_idx, Arc::clone(&asset_arc));
          }
          output.push(Output::Asset(asset_arc));
        }
        InstantiationKind::None => {
          output.push(Output::Asset(Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: vec![],
            names: vec![],
          })));
        }
      }
    }

    if let Some(cache) = self.cache.as_deref_mut() {
      cache.last_reused_chunk_filenames.clear();
      let omit_bytes = cache.omit_reused_chunk_bytes;
      for &chunk_idx in reused_chunks {
        let module_idx = chunk_graph.chunk_table[chunk_idx].modules[0];
        if let Some(cached) = cache.cached_native_chunks.get(&module_idx) {
          cache.last_reused_chunk_filenames.insert(cached.chunk.filename.clone());
          output.push(Output::Chunk(Arc::clone(&cached.chunk)));
          if !omit_bytes && let Some(map_asset) = &cached.sourcemap_asset {
            output.push(Output::Asset(Arc::clone(map_asset)));
          }
        }
      }
      for (chunk_idx, chunk_arc) in newly_cached_chunks {
        if let Some(decision_hash) = chunk_decisions.get(chunk_idx).and_then(|d| *d) {
          let module_idx = chunk_graph.chunk_table[chunk_idx].modules[0];
          let (cached_chunk, sourcemap_asset) = if omit_bytes {
            (
              Arc::new(OutputChunk {
                name: chunk_arc.name.clone(),
                filename: chunk_arc.filename.clone(),
                code: String::new(),
                is_entry: chunk_arc.is_entry,
                is_dynamic_entry: chunk_arc.is_dynamic_entry,
                facade_module_id: chunk_arc.facade_module_id.clone(),
                modules: FxHashMap::default().into(),
                exports: vec![],
                module_ids: vec![],
                // Relink consumers still need the complete static-import graph.
                imports: chunk_arc.imports.clone(),
                dynamic_imports: vec![],
                map: None,
                sourcemap_filename: chunk_arc.sourcemap_filename.clone(),
                preliminary_filename: chunk_arc.preliminary_filename.clone(),
              }),
              None,
            )
          } else {
            (chunk_arc, newly_cached_maps.remove(&chunk_idx))
          };
          cache.cached_native_chunks.insert(
            module_idx,
            CachedNativeChunk { decision_hash, chunk: cached_chunk, sourcemap_asset },
          );
        }
      }
      let live_single_modules: FxHashSet<ModuleIdx> = chunk_graph
        .chunk_table
        .iter()
        .filter_map(|c| (c.modules.len() == 1).then(|| c.modules[0]))
        .collect();
      cache.cached_native_chunks.retain(|idx, _| live_single_modules.contains(idx));
      cache.cached_chunk_namings.retain(|idx, _| live_single_modules.contains(idx));
    }

    // The chunks order make sure the entry chunk at first, the assets at last, see https://github.com/rollup/rollup/blob/master/src/rollup/rollup.ts#L266
    output.sort_unstable_by(|a, b| {
      let a_type = get_sorting_file_type(a) as u8;
      let b_type = get_sorting_file_type(b) as u8;
      if a_type == b_type {
        return a.filename().cmp(b.filename());
      }
      a_type.cmp(&b_type)
    });

    // Now that all diagnostics are collected, drain by severity: any error
    // aborts the build, otherwise the warnings ride out on the `BundleOutput`.
    // `into_result` fast-paths the common no-error case, skipping the partition.
    let warnings = diagnostics.into_result()?;
    Ok(BundleOutput { assets: output, warnings, mangle_cache })
  }

  fn prepare_chunk_sourcemap_cache(
    &mut self,
    chunk_graph: &ChunkGraph,
    rendered_modules: &IndexVec<ModuleIdx, Option<ModuleRenderOutput>>,
    _order_state: &super::order_wrap_state::OrderWrapState,
    reused_chunks: &FxHashSet<ChunkIdx>,
  ) -> (FxHashSet<ChunkIdx>, FxHashMap<ChunkIdx, [u128; 2]>) {
    use std::hash::Hash;
    use xxhash_rust::xxh3::Xxh3;

    let mut skip_chunks = FxHashSet::default();
    let mut multi_module_chunk_bases = FxHashMap::default();
    let link_output = &*self.link_output;
    let options = self.options;
    let Some(cache) = self.cache.as_deref_mut() else {
      return (skip_chunks, multi_module_chunk_bases);
    };
    cache.current_chunk_map_digests.clear();
    cache.current_chunk_minify_state.clear();
    cache.pre_redacted_maps.clear();

    let eligible = matches!(
      options.sourcemap,
      Some(rolldown_common::SourceMapType::File | rolldown_common::SourceMapType::Hidden) | None
    ) && !options.sourcemap_debug_ids
      && !options.is_dev_mode_enabled()
      && options.post_banner.is_none()
      && options.post_footer.is_none()
      && matches!(
        options.format,
        rolldown_common::OutputFormat::Esm | rolldown_common::OutputFormat::Cjs
      );
    if !eligible {
      return (skip_chunks, multi_module_chunk_bases);
    }

    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if reused_chunks.contains(&chunk_idx)
        || chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
        || self.inline_state.is_record(chunk_idx)
        || !self.inline_state.carried_by(chunk_idx).is_empty()
      {
        continue;
      }
      if !chunk.module_groups.is_empty()
        || !chunk.entry_level_external_module_idx.is_empty()
      {
        continue;
      }
      let kind_key = match &chunk.kind {
        rolldown_common::ChunkKind::EntryPoint { meta, module, .. } => {
          (1u8, meta.bits(), module.raw())
        }
        rolldown_common::ChunkKind::Common => (0u8, 0, 0),
      };
      let mut all_have_digest = true;
      // Every module's rendered code is a cached finalize result, which its digest determines.
      let mut all_finalize_digests = true;
      let mut all_hit = true;
      let mut h0 = Xxh3::with_seed(0x9e3779b97f4a7c15);
      let mut h1 = Xxh3::with_seed(0x6a09e667f3bcc908);
      let hash_norm_filename = |f: Option<&str>, h0: &mut Xxh3, h1: &mut Xxh3| match f {
        None => {
          0u8.hash(h0);
          0u8.hash(h1);
        }
        Some(s) => {
          1u8.hash(h0);
          1u8.hash(h1);
          rolldown_utils::hash_placeholder::visit_with_placeholders_defaulted(
            s,
            &rolldown_utils::hash_placeholder::HASH_PLACEHOLDER_LEFT_FINDER,
            |_| true,
            |bytes| {
              h0.update(bytes);
              h1.update(bytes);
            },
          );
        }
      };
      let header = (
        kind_key,
        options.format as u8,
        chunk.output_exports as u8,
        options.strict as u8,
        options.experimental.is_attach_debug_info_full(),
        chunk.debug_info.len(),
        chunk.imports_from_other_chunks.len(),
        chunk.import_symbol_from_external_modules.len(),
        chunk.direct_imports_from_external_modules.len(),
        chunk.modules.len(),
      );
      header.hash(&mut h0);
      header.hash(&mut h1);
      hash_norm_filename(
        chunk.preliminary_filename.as_ref().map(|f| f.as_str()),
        &mut h0,
        &mut h1,
      );
      // The multi-module chunk cache keys rendered imports and exports exactly, so its base
      // leaves out the symbol indices below, which a partial relink renumbers.
      let (mut b0, mut b1) = (h0.clone(), h1.clone());
      if options.minify.is_enabled() {
        for (exporter_id, items) in &chunk.imports_from_other_chunks {
          hash_norm_filename(
            chunk_graph.chunk_table[*exporter_id]
              .preliminary_filename
              .as_ref()
              .map(|f| f.as_str()),
            &mut h0,
            &mut h1,
          );
          let v = (
            chunk
              .require_binding_names_for_other_chunks
              .get(exporter_id)
              .map(|s| s.as_str()),
            items.is_empty(),
            items.len(),
          );
          v.hash(&mut h0);
          v.hash(&mut h1);
          for item in items {
            let iv = (
              item.import_ref.owner.raw(),
              item.import_ref.symbol.index() as u32,
            );
            iv.hash(&mut h0);
            iv.hash(&mut h1);
          }
        }
        let mut exp_acc = (0u128, 0u128);
        for (sym_ref, names) in &chunk.exports_to_other_chunks {
          let mut eh0 = Xxh3::with_seed(0x9e3779b97f4a7c15);
          let mut eh1 = Xxh3::with_seed(0x6a09e667f3bcc908);
          let ev = (sym_ref.owner.raw(), sym_ref.symbol.index() as u32, names.as_slice());
          ev.hash(&mut eh0);
          ev.hash(&mut eh1);
          exp_acc = (
            exp_acc.0.wrapping_add(eh0.digest128()),
            exp_acc.1.wrapping_add(eh1.digest128()),
          );
        }
        (chunk.exports_to_other_chunks.len(), exp_acc).hash(&mut h0);
        (chunk.exports_to_other_chunks.len(), exp_acc).hash(&mut h1);
      }
      let (h0_head, h1_head) = (h0, h1);
      let mut h0 = Xxh3::with_seed(0x9e3779b97f4a7c15);
      let mut h1 = Xxh3::with_seed(0x6a09e667f3bcc908);
      for (ext_id, named) in &chunk.direct_imports_from_external_modules {
        let v = (link_output.module_table[*ext_id].id().as_str(), named.len());
        v.hash(&mut h0);
        v.hash(&mut h1);
      }
      for &m_idx in &chunk.modules {
        if !link_output.module_table[m_idx].is_normal() || !link_output.metas[m_idx].is_included {
          continue;
        }
        if let Some(&m_digest) = cache.current_module_finalize_digests.get(&m_idx) {
          if !cache.current_module_was_hit.contains(&m_idx) {
            all_hit = false;
          }
          (1u8, m_idx.raw(), m_digest).hash(&mut h0);
          (1u8, m_idx.raw(), m_digest).hash(&mut h1);
        } else if let Some(ro) = rendered_modules.get(m_idx).and_then(Option::as_ref) {
          all_finalize_digests = false;
          let is_file_url_asset = link_output.module_table[m_idx]
            .as_normal()
            .is_some_and(|m| !m.ecma_view.rolldown_file_url_references.is_empty());
          let scan_gen = if is_file_url_asset { 1 } else { cache.module_scan_generation(m_idx) };
          let v = (2u8, m_idx.raw(), scan_gen, ro.code.as_str());
          v.hash(&mut h0);
          v.hash(&mut h1);
        } else if cache.current_rendered_sources.contains_key(&m_idx) {
          all_have_digest = false;
          break;
        } else {
          all_finalize_digests = false;
          (0u8, m_idx.raw()).hash(&mut h0);
          (0u8, m_idx.raw()).hash(&mut h1);
        }
      }
      if !all_have_digest {
        continue;
      }
      let tail = [h0.digest128(), h1.digest128()];
      let (mut h0, mut h1) = (h0_head, h1_head);
      for hasher in [&mut h0, &mut h1, &mut b0, &mut b1] {
        tail.hash(hasher);
      }
      let digest = [h0.digest128(), h1.digest128()];
      cache.current_chunk_map_digests.insert(chunk_idx, digest);
      if all_finalize_digests && !trace_action_enabled!() {
        // Instantiating keys the chunk exactly, then decides on its source map by that key.
        multi_module_chunk_bases.insert(chunk_idx, [b0.digest128(), b1.digest128()]);
      } else if all_hit && cache.cached_chunk_sourcemaps.contains_key(&digest) {
        skip_chunks.insert(chunk_idx);
      }
    }
    (skip_chunks, multi_module_chunk_bases)
  }

  /// Caches the chunk code that instantiating rendered, forgets the entries that this build did
  /// not render, and returns each rendered chunk's key. A rendered chunk's key also keys its
  /// source map and minify state from here on.
  fn cache_multi_module_chunks(
    &mut self,
    renders: Vec<(ChunkIdx, MultiModuleChunkRender)>,
    skip_sourcemap_chunks: &mut FxHashSet<ChunkIdx>,
  ) -> FxHashMap<ChunkIdx, [u128; 2]> {
    let mut chunks = FxHashMap::with_capacity_and_hasher(renders.len(), Default::default());
    let Some(cache) = self.cache.as_deref_mut() else {
      return chunks;
    };
    let mut live = FxHashSet::with_capacity_and_hasher(renders.len(), Default::default());
    for (chunk_idx, render) in renders {
      if let Some(entry) = render.new_entry {
        cache.cached_multi_module_chunks.insert(render.key, entry);
      }
      live.insert(render.key);
      cache.current_chunk_map_digests.insert(chunk_idx, render.key);
      if render.skip_sourcemap_join {
        skip_sourcemap_chunks.insert(chunk_idx);
      }
      chunks.insert(chunk_idx, render.key);
    }
    cache.cached_multi_module_chunks.retain(|key, _| live.contains(key));
    cache.current_multi_module_chunk_keys = live;
    chunks
  }

  #[allow(clippy::too_many_arguments)]
  #[tracing::instrument(level = "debug", skip_all)]
  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    mut rendered_modules: IndexVec<ModuleIdx, Option<ModuleRenderOutput>>,
    diagnostics: &mut Diagnostics,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &super::Sealed<super::FinalEsmInitMetadata>,
    reused_chunks: &FxHashSet<ChunkIdx>,
    skip_sourcemap_chunks: &FxHashSet<ChunkIdx>,
    multi_module_chunk_bases: &FxHashMap<ChunkIdx, [u128; 2]>,
    mut inline_renders: FxHashMap<ChunkIdx, Vec<CarriedRender>>,
  ) -> BuildResult<(
    IndexInstantiatedChunks,
    IndexChunkToInstances,
    Vec<(ChunkIdx, MultiModuleChunkRender)>,
  )> {
    let mut index_chunk_to_instances: IndexChunkToInstances =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
    let mut index_instantiated_chunks: IndexInstantiatedChunks = IndexVec::with_capacity(
      chunk_graph.chunk_table.len().saturating_sub(reused_chunks.len()),
    );
    let chunk_index_to_codegen_rets =
      self.create_chunk_to_codegen_ret_map(chunk_graph, &mut rendered_modules, reused_chunks);
    let mut needed_exporter_chunks = FxHashSet::default();
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if reused_chunks.contains(&chunk_idx)
        || chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
        || self.inline_state.is_record(chunk_idx)
      {
        continue;
      }
      needed_exporter_chunks.insert(chunk_idx);
      needed_exporter_chunks.extend(chunk.imports_from_other_chunks.keys().copied());
      needed_exporter_chunks.extend(self.inline_state.carried_by(chunk_idx).iter().copied());
    }
    for record_idx in self.inline_state.record_indices() {
      needed_exporter_chunks.insert(record_idx);
    }
    let render_export_items_index_vec: IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>> =
      chunk_graph
        .chunk_table
        .chunks
        .par_iter_enumerated()
        .map(|(chunk_idx, item)| {
          let mut map: FxIndexMap<SymbolRef, Vec<CompactStr>> = FxIndexMap::default();
          if needed_exporter_chunks.contains(&chunk_idx) {
            get_export_items(item).into_iter().for_each(|(k, v)| {
              map.entry(v).or_default().push(k);
            });
          }
          map
        })
        .collect::<Vec<_>>()
        .into();
    let render_export_items_index_vec = &render_export_items_index_vec;

    let can_parallel_instantiate = self.options.post_banner.is_none()
      && self.options.post_footer.is_none()
      && matches!(
        self.options.format,
        rolldown_common::OutputFormat::Esm | rolldown_common::OutputFormat::Cjs
      );

    let items: Vec<_> = chunk_index_to_codegen_rets
      .into_iter()
      .enumerate()
      .filter_map(|(idx, module_id_to_codegen_ret)| {
        let chunk_idx =
          ChunkIdx::from_raw(u32::try_from(idx).expect("chunk index should fit in u32"));
        // A record is no file; its modules are printed by the files that carry it.
        if reused_chunks.contains(&chunk_idx)
          || chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
          || self.inline_state.is_record(chunk_idx)
        {
          return None;
        }
        let chunk = chunk_graph.chunk_table.get(chunk_idx)?;
        let carried_renders = inline_renders.remove(&chunk_idx).unwrap_or_default();
        Some((chunk_idx, chunk, module_id_to_codegen_ret, carried_renders))
      })
      .collect();

    let results = if can_parallel_instantiate {
      use rolldown_utils::rayon::{IntoParallelIterator, ParallelIterator};
      let resolved_paths = self.resolved_paths.as_ref();
      let cached_module_sources = self.cache.as_ref().map(|c| &c.current_rendered_sources);
      let cache = self.cache.as_deref();
      items
        .into_par_iter()
        .map(|(chunk_idx, chunk, module_id_to_codegen_ret, carried_renders)| {
          let multi_module_chunk = cache.and_then(|cache| {
            Some(MultiModuleChunkLookup {
              base_digest: *multi_module_chunk_bases.get(&chunk_idx)?,
              cached: &cache.cached_multi_module_chunks,
              cached_sourcemaps: &cache.cached_chunk_sourcemaps,
            })
          });
          let mut ecma_ctx = GenerateContext {
            chunk_idx,
            chunk,
            options: self.options,
            link_output: self.link_output,
            used_symbol_refs,
            order_wrap_state: order_state,
            chunk_graph,
            plugin_driver: self.plugin_driver,
            module_id_to_codegen_ret,
            cached_module_sources,
            skip_sourcemap_join: skip_sourcemap_chunks.contains(&chunk_idx),
            multi_module_chunk,
            render_export_items_index_vec,
            resolved_paths,
            ast_table: Some(&self.ast_table),
            final_esm_init_metadata: Some(final_esm_init_metadata),
            inline_state: &self.inline_state,
            carried_renders,
          };
          crate::utils::process_code_and_sourcemap::poll_ready_or_block_on(
            EcmaGenerator::instantiate_chunk(&mut ecma_ctx),
          )
        })
        .collect::<anyhow::Result<Vec<_>>>()?
    } else {
      try_join_all(
        items
          .into_iter()
          .flat_map(|(chunk_idx, chunk, module_id_to_codegen_ret, carried_renders)| {
            let resolved_paths = self.resolved_paths.as_ref();
            let cached_module_sources =
              self.cache.as_ref().map(|c| &c.current_rendered_sources);
            let skip_sourcemap_join = skip_sourcemap_chunks.contains(&chunk_idx);
            let ast_table = &self.ast_table;
            let inline_state = &self.inline_state;
            let ecma_chunks_future: ChunkGeneratorFuture = Box::pin(async move {
              let mut ecma_ctx = GenerateContext {
                chunk_idx,
                chunk,
                options: self.options,
                link_output: self.link_output,
                used_symbol_refs,
                order_wrap_state: order_state,
                chunk_graph,
                plugin_driver: self.plugin_driver,
                module_id_to_codegen_ret,
                cached_module_sources,
                skip_sourcemap_join,
                multi_module_chunk: None,
                render_export_items_index_vec,
                resolved_paths,
                ast_table: Some(ast_table),
                final_esm_init_metadata: Some(final_esm_init_metadata),
                inline_state,
                carried_renders,
              };
              let ecma_chunks_future = EcmaGenerator::instantiate_chunk(&mut ecma_ctx);
              let ecma_chunks = ecma_chunks_future.await?;
              Ok(ecma_chunks)
            });
            [ecma_chunks_future]
          }),
      )
      .await?
    };

    let mut multi_module_renders = Vec::new();
    results
      .into_iter()
      .for_each(|result| match result {
      Ok(generate_output) => {
        if let Some(render) = generate_output.multi_module_chunk
          && let Some(ins_chunk) = generate_output.chunks.first()
        {
          multi_module_renders.push((ins_chunk.originate_from, render));
        }
        generate_output.chunks.into_iter().for_each(|ins_chunk| {
          let base_chunk_idx = ins_chunk.originate_from;
          let ins_chunk_idx = index_instantiated_chunks.push(ins_chunk);
          index_chunk_to_instances[base_chunk_idx].insert(ins_chunk_idx);
        });
        diagnostics.extend(generate_output.warnings);
      }
      Err(e) => diagnostics.extend(e.into_vec()),
    });

    index_chunk_to_instances.iter_mut().for_each(|instances| {
      if instances.len() > 1 {
        instances.sort_by_cached_key(|ins_chunk_idx| {
          index_instantiated_chunks[*ins_chunk_idx].preliminary_filename.as_str()
        });
      }
    });

    Ok((index_instantiated_chunks, index_chunk_to_instances, multi_module_renders))
  }

  fn create_chunk_to_codegen_ret_map(
    &self,
    chunk_graph: &ChunkGraph,
    rendered_modules: &mut IndexVec<ModuleIdx, Option<ModuleRenderOutput>>,
    reused_chunks: &FxHashSet<ChunkIdx>,
  ) -> Vec<Vec<Option<ModuleRenderOutput>>> {
    chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, item)| {
        // A record's modules were finalized and printed per carrier; its own ASTs stay untouched.
        if reused_chunks.contains(&chunk_idx) || self.inline_state.is_record(chunk_idx) {
          return Vec::new();
        }
        item
          .modules
          .iter()
          .map(|&module_idx| rendered_modules[module_idx].take())
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>()
  }

  fn trace_action_assets_ready(index_assets: &AssetVec) {
    if trace_action_enabled!() {
      let mut assets = vec![];
      for asset in index_assets {
        assets.push(action::Asset {
          chunk_id: matches!(asset.meta, InstantiationKind::Ecma(_))
            .then_some(asset.originate_from)
            .flatten()
            .map(ChunkIdx::raw),
          content: asset.content.try_as_inner_str().ok().map(str::to_string),
          size: asset.content.as_bytes().len().try_into().unwrap(),
          filename: asset.filename.to_string(),
        });
      }
      let assets_ready = action::AssetsReady { action: "AssetsReady", assets };
      trace_action!(assets_ready);
    }
  }
}

enum SortingFileType {
  EntryChunk = 0,
  SecondaryChunk = 1,
  Asset = 2,
}

pub(super) fn get_sorting_file_type_key(output: &Output) -> u8 {
  get_sorting_file_type(output) as u8
}

#[inline]
fn get_sorting_file_type(output: &Output) -> SortingFileType {
  match output {
    Output::Asset(_) => SortingFileType::Asset,
    Output::Chunk(chunk) => {
      if chunk.is_entry {
        SortingFileType::EntryChunk
      } else {
        SortingFileType::SecondaryChunk
      }
    }
  }
}

pub fn set_emitted_chunk_preliminary_filenames(
  file_emitter: &SharedFileEmitter,
  chunk_graph: &ChunkGraph,
) {
  let emitted_chunk_info = chunk_graph
    .chunk_idx_to_reference_ids
    .iter()
    .flat_map(|(&idx, reference_ids)| {
      let chunk = &chunk_graph.chunk_table.chunks[idx];
      reference_ids.iter().map(|reference_id| EmittedChunkInfo {
        reference_id: reference_id.clone(),
        filename: chunk
          .preliminary_filename
          .as_ref()
          .expect("Emitted chunk should have filename")
          .deref()
          .clone(),
      })
    });
  file_emitter.set_emitted_chunk_info(emitted_chunk_info);
}

impl GenerateStage<'_> {
  #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
  pub(super) async fn render_dirty_chunks_to_assets(
    &mut self,
    chunk_graph: &ChunkGraph,
    dirty_rendered_modules: Vec<(ChunkIdx, Vec<Option<ModuleRenderOutput>>)>,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    chunk_decisions: &IndexVec<ChunkIdx, Option<[u128; 2]>>,
    render_export_items_index_vec: &mut IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>>,
    index_chunk_to_instances: &mut IndexChunkToInstances,
    sorted_output_template: &mut Vec<(ChunkIdx, Output)>,
    chunk_idx_to_template_pos: &IndexVec<ChunkIdx, Option<usize>>,
    reused_chunk_filenames_pool: &mut FxHashSet<ArcStr>,
    excluded_from_reused_filenames: &mut Vec<ArcStr>,
    pool_installed: &mut bool,
  ) -> BuildResult<BundleOutput> {
    let mut diagnostics = std::mem::take(&mut self.link_output.diagnostics);
    let dirty_chunk_indices: Vec<ChunkIdx> =
      dirty_rendered_modules.iter().map(|(c, _)| *c).collect();

    for &(chunk_idx, _) in &dirty_rendered_modules {
      let map = &mut render_export_items_index_vec[chunk_idx];
      map.clear();
      for (k, v) in get_export_items(&chunk_graph.chunk_table[chunk_idx]) {
        map.entry(v).or_default().push(k);
      }
      for &importee_chunk_idx in chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys()
      {
        if render_export_items_index_vec[importee_chunk_idx].is_empty()
          && !chunk_graph.chunk_table[importee_chunk_idx].exports_to_other_chunks.is_empty()
        {
          let imp_map = &mut render_export_items_index_vec[importee_chunk_idx];
          for (k, v) in get_export_items(&chunk_graph.chunk_table[importee_chunk_idx]) {
            imp_map.entry(v).or_default().push(k);
          }
        }
      }
    }

    let mut instantiated_chunks: IndexInstantiatedChunks =
      IndexVec::with_capacity(dirty_rendered_modules.len());
    {
      let render_export_items_ref: &IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>> =
        render_export_items_index_vec;
      let resolved_paths = self.resolved_paths.as_ref();
      let options = self.options;
      let link_output: &crate::stages::link_stage::LinkStageOutput = self.link_output;
      let plugin_driver = self.plugin_driver;
      let inline_state = &self.inline_state;
      let futures = dirty_rendered_modules.into_iter().map(|(chunk_idx, module_id_to_codegen_ret)| {
        let chunk = &chunk_graph.chunk_table[chunk_idx];
        let ecma_chunks_future: ChunkGeneratorFuture = Box::pin(async move {
          let mut ecma_ctx = GenerateContext {
            chunk_idx,
            chunk,
            options,
            link_output,
            used_symbol_refs,
            order_wrap_state: order_state,
            chunk_graph,
            plugin_driver,
            module_id_to_codegen_ret,
            cached_module_sources: None,
            skip_sourcemap_join: false,
            multi_module_chunk: None,
            render_export_items_index_vec: render_export_items_ref,
            resolved_paths,
            ast_table: None,
            final_esm_init_metadata: None,
            inline_state,
            carried_renders: vec![],
          };
          EcmaGenerator::instantiate_chunk(&mut ecma_ctx).await
        });
        ecma_chunks_future
      });
      for result in try_join_all(futures).await? {
        match result {
          Ok(generate_output) => {
            for ins_chunk in generate_output.chunks {
              let base_chunk_idx = ins_chunk.originate_from;
              let ins_chunk_idx = instantiated_chunks.push(ins_chunk);
              index_chunk_to_instances[base_chunk_idx].insert(ins_chunk_idx);
            }
            diagnostics.extend(generate_output.warnings);
          }
          Err(e) => diagnostics.extend(e.into_vec()),
        }
      }
    }

    for &chunk_idx in &dirty_chunk_indices {
      if index_chunk_to_instances[chunk_idx].len() > 1 {
        index_chunk_to_instances[chunk_idx].sort_by_cached_key(|ins_chunk_idx| {
          instantiated_chunks[*ins_chunk_idx].preliminary_filename.as_str()
        });
      }
    }

    let runtime_chunk_id = chunk_graph.module_to_chunk[self.link_output.runtime.id()];
    for ins_chunk in &instantiated_chunks {
      if Some(ins_chunk.originate_from) == runtime_chunk_id
        && let InstantiationKind::Ecma(ecma_meta) = &ins_chunk.kind
        && let Some(cache) = self.cache.as_deref_mut()
      {
        cache.runtime_rendered_chunk = Some(Arc::clone(&ecma_meta.rendered_chunk));
      }
    }
    let extra_runtime_chunk =
      self.cache.as_ref().and_then(|cache| cache.runtime_rendered_chunk.as_ref());

    let render_res = render_chunks(
      self.plugin_driver,
      &mut instantiated_chunks,
      self.options,
      &chunk_graph.chunk_table,
      &self.link_output.module_table,
      extra_runtime_chunk,
    )
    .await;
    if let Err(err) = render_res {
      for &chunk_idx in &dirty_chunk_indices {
        index_chunk_to_instances[chunk_idx].clear();
      }
      return Err(err.into());
    }
    diagnostics.extend(render_res?);

    if let Err(err) = augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await {
      for &chunk_idx in &dirty_chunk_indices {
        index_chunk_to_instances[chunk_idx].clear();
      }
      return Err(err.into());
    }

    let mangle_cache = match self.minify_chunks(&mut instantiated_chunks, &FxHashSet::default()) {
      Ok(v) => v,
      Err(err) => {
        for &chunk_idx in &dirty_chunk_indices {
          index_chunk_to_instances[chunk_idx].clear();
        }
        return Err(err.into());
      }
    };

    if let Err(err) = Self::post_banner_footer(&mut instantiated_chunks) {
      for &chunk_idx in &dirty_chunk_indices {
        index_chunk_to_instances[chunk_idx].clear();
      }
      return Err(err.into());
    }

    let assets_res = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated_chunks,
      index_chunk_to_instances,
      self.options.hash_characters,
      self.options,
      self.resolved_paths.as_ref(),
      None,
      self.cache.as_deref_mut(),
    )
    .await;
    for &chunk_idx in &dirty_chunk_indices {
      index_chunk_to_instances[chunk_idx].clear();
    }
    let assets = assets_res?;

    set_emitted_chunk_filenames(&self.plugin_driver.file_emitter, &assets, chunk_graph);

    let mut newly_cached_chunks: Vec<(ChunkIdx, Arc<OutputChunk>)> =
      Vec::with_capacity(dirty_chunk_indices.len());
    let mut extra_assets: Vec<Output> = Vec::new();
    for Asset { map, meta: rendered_chunk, content: code, filename, originate_from } in assets {
      match rendered_chunk {
        InstantiationKind::Ecma(ecma_meta) => {
          let code = code.try_into_string()?;
          let rendered_chunk = ecma_meta.rendered_chunk;
          let chunk_arc = Arc::new(OutputChunk {
            name: rendered_chunk.name.clone(),
            filename: filename.clone(),
            code,
            is_entry: rendered_chunk.is_entry,
            is_dynamic_entry: rendered_chunk.is_dynamic_entry,
            facade_module_id: rendered_chunk.facade_module_id.clone(),
            modules: FxHashMap::default().into(),
            exports: rendered_chunk.exports.clone(),
            module_ids: rendered_chunk.module_ids.clone(),
            imports: ecma_meta.imports,
            dynamic_imports: ecma_meta.dynamic_imports,
            map,
            sourcemap_filename: ecma_meta.sourcemap_filename,
            preliminary_filename: ecma_meta.preliminary_filename.to_string(),
          });
          if let Some(chunk_idx) = originate_from {
            newly_cached_chunks.push((chunk_idx, chunk_arc));
          }
        }
        InstantiationKind::Sourcemap(sourcemap_meta) => {
          let asset_arc = Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: sourcemap_meta.original_file_names,
            names: sourcemap_meta.names,
          });
          extra_assets.push(Output::Asset(asset_arc));
        }
        InstantiationKind::None => {
          extra_assets.push(Output::Asset(Arc::new(OutputAsset {
            filename: filename.clone(),
            source: code,
            original_file_names: vec![],
            names: vec![],
          })));
        }
      }
    }

    if let Some(cache) = self.cache.as_deref_mut() {
      cache.last_rebuilt_chunk_filenames.clear();
      cache.last_rebuilt_chunk_outputs.clear();
      if !*pool_installed {
        cache.last_reused_chunk_filenames = std::mem::take(reused_chunk_filenames_pool);
        *pool_installed = true;
      }
      for prev_filename in excluded_from_reused_filenames.drain(..) {
        cache.last_reused_chunk_filenames.insert(prev_filename);
      }
      for &(chunk_idx, ref chunk_arc) in &newly_cached_chunks {
        cache.last_rebuilt_chunk_filenames.push(chunk_arc.filename.clone());
        cache.last_rebuilt_chunk_outputs.push(Output::Chunk(Arc::clone(chunk_arc)));
        cache.last_reused_chunk_filenames.remove(&chunk_arc.filename);
        excluded_from_reused_filenames.push(chunk_arc.filename.clone());
        if let Some(decision_hash) = chunk_decisions.get(chunk_idx).and_then(|d| *d) {
          let module_idx = chunk_graph.chunk_table[chunk_idx].modules[0];
          let cached_chunk = Arc::new(OutputChunk {
            name: chunk_arc.name.clone(),
            filename: chunk_arc.filename.clone(),
            code: String::new(),
            is_entry: chunk_arc.is_entry,
            is_dynamic_entry: chunk_arc.is_dynamic_entry,
            facade_module_id: chunk_arc.facade_module_id.clone(),
            modules: FxHashMap::default().into(),
            exports: vec![],
            module_ids: vec![],
            imports: vec![],
            dynamic_imports: vec![],
            map: None,
            sourcemap_filename: chunk_arc.sourcemap_filename.clone(),
            preliminary_filename: chunk_arc.preliminary_filename.clone(),
          });
          if let Some(pos) = chunk_idx_to_template_pos.get(chunk_idx).and_then(|p| *p) {
            sorted_output_template[pos].1 = Output::Chunk(Arc::clone(&cached_chunk));
          }
          cache.cached_native_chunks.insert(
            module_idx,
            CachedNativeChunk { decision_hash, chunk: cached_chunk, sourcemap_asset: None },
          );
        }
      }
      for extra in &extra_assets {
        cache.last_rebuilt_chunk_filenames.push(extra.filename().into());
        cache.last_rebuilt_chunk_outputs.push(extra.clone());
      }
    }

    let mut output: Vec<Output> =
      Vec::with_capacity(sorted_output_template.len() + extra_assets.len());
    output.extend(sorted_output_template.iter().map(|(_, out)| out.clone()));
    for (chunk_idx, chunk_arc) in newly_cached_chunks {
      if let Some(pos) = chunk_idx_to_template_pos.get(chunk_idx).and_then(|p| *p) {
        output[pos] = Output::Chunk(chunk_arc);
      }
    }
    if !extra_assets.is_empty() {
      extra_assets.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));
      output.extend(extra_assets);
    }

    let warnings = diagnostics.into_result()?;
    Ok(BundleOutput { assets: output, warnings, mangle_cache })
  }
}

fn set_emitted_chunk_filenames(
  file_emitter: &SharedFileEmitter,
  assets: &AssetVec,
  chunk_graph: &ChunkGraph,
) {
  let emitted_chunk_info = assets
    .iter()
    .filter(|asset| matches!(asset.meta, InstantiationKind::Ecma(_)))
    .filter_map(|asset| {
      asset.originate_from.and_then(|originate_from| {
        chunk_graph.chunk_idx_to_reference_ids.get(&originate_from).map(|reference_ids| {
          reference_ids.iter().map(|reference_id| EmittedChunkInfo {
            reference_id: reference_id.clone(),
            filename: asset.filename.clone(),
          })
        })
      })
    })
    .flatten();
  file_emitter.set_emitted_chunk_info(emitted_chunk_info);
}
