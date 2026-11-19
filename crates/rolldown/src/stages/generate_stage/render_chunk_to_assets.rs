use std::{ops::Deref, sync::Arc};

use futures::future::try_join_all;
use oxc_index::{IndexVec, index_vec};
use oxc_str::CompactStr;
use rolldown_common::{
  Asset, ChunkIdx, EmittedChunkInfo, InstantiationKind, ModuleIdx, ModuleRenderOutput, Output,
  OutputAsset, OutputChunk, SharedFileEmitter, SymbolRef, UsedSymbolRefs,
};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildResult, Diagnostics};
use rolldown_utils::{
  index_vec_ext::IndexVecExt as _,
  indexmap::FxIndexMap,
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use crate::{
  BundleOutput,
  chunk_graph::ChunkGraph,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{AssetVec, IndexChunkToInstance, IndexEcmaAst, IndexInstantiatedChunks},
  types::generator::GenerateContext,
  utils::{
    augment_chunk_hash::augment_chunk_hash,
    chunk::{finalize_chunks::finalize_assets, render_chunk_exports::get_export_items},
    render_chunks::render_chunks,
  },
};

use super::{CarriedRender, GenerateStage, minify_chunks::ChunkMinifier};

impl GenerateStage<'_> {
  pub async fn render_chunk_to_assets(
    &mut self,
    chunk_graph: &ChunkGraph,
    ast_table: IndexEcmaAst,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
  ) -> BuildResult<BundleOutput> {
    // Move the mixed-severity accumulator out of `link_output` so it can be
    // passed by `&mut` to `instantiate_chunks` (which borrows `&self`, and so
    // cannot also borrow `self.link_output.diagnostics`). The split into
    // warnings vs. errors is deferred to the end of the function, where their
    // fates finally diverge (errors -> `Err`, warnings -> `BundleOutput`).
    let mut diagnostics = std::mem::take(&mut self.link_output.diagnostics);
    let inline_renders = std::mem::take(&mut self.inline_renders);
    // `ast_table` is threaded by value into `create_chunk_to_codegen_ret_map`
    // (the last reader), which frees each module's AST as soon as it is printed
    // and drops the rest at scope exit — releasing the per-module bumpalo arenas
    // before minification (in `render_chunks`) and `finalize_assets` allocate.
    let (mut instantiated_chunks, index_chunk_to_instance) = self
      .instantiate_chunks(
        chunk_graph,
        ast_table,
        &mut diagnostics,
        used_symbol_refs,
        order_state,
        inline_renders,
      )
      .await?;

    self.trace_action_package_graph_ready(chunk_graph, &instantiated_chunks);
    self.trace_action_module_rendered(&instantiated_chunks);

    // Chunks are minified inside `render_chunks`, each right after its `renderChunk` hooks.
    // `augmentChunkHash` runs afterwards; minification changes nothing that hook receives.
    let minifier = ChunkMinifier::new(self.options, &instantiated_chunks)?.map(Arc::new);
    diagnostics.extend(
      render_chunks(
        self.plugin_driver,
        &mut instantiated_chunks,
        self.options,
        &chunk_graph.chunk_table,
        &self.link_output.module_table,
        minifier.as_ref(),
      )
      .await?,
    );

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let mangle_cache = minifier.and_then(|minifier| {
      Arc::into_inner(minifier).expect("minify jobs have finished").into_mangle_cache()
    });

    Self::post_banner_footer(&mut instantiated_chunks)?;

    let assets = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated_chunks,
      &index_chunk_to_instance,
      self.options.hash_characters,
      self.options,
      self.resolved_paths.as_ref(),
    )
    .await?;

    // Set emitted chunk info for file emitter, it should be set before call generate_bundle hook
    set_emitted_chunk_filenames(&self.plugin_driver.file_emitter, &assets, chunk_graph);

    Self::trace_action_assets_ready(&assets);

    let mut output = Vec::with_capacity(assets.len());
    for Asset { map, meta: rendered_chunk, content: code, filename, .. } in assets {
      match rendered_chunk {
        InstantiationKind::Ecma(ecma_meta) => {
          let code = code.try_into_string()?;
          let rendered_chunk = ecma_meta.rendered_chunk;
          output.push(Output::Chunk(Arc::new(OutputChunk {
            name: rendered_chunk.name.clone(),
            filename,
            code,
            is_entry: rendered_chunk.is_entry,
            is_dynamic_entry: rendered_chunk.is_dynamic_entry,
            facade_module_id: rendered_chunk.facade_module_id.clone(),
            modules: rendered_chunk.modules.clone(),
            exports: rendered_chunk.exports.clone(),
            module_ids: rendered_chunk.module_ids.clone(),
            imports: ecma_meta.imports,
            dynamic_imports: ecma_meta.dynamic_imports,
            map,
            sourcemap_filename: ecma_meta.sourcemap_filename,
            preliminary_filename: ecma_meta.preliminary_filename.to_string(),
          })));
        }
        InstantiationKind::Sourcemap(sourcemap_meta) => {
          output.push(Output::Asset(Arc::new(OutputAsset {
            filename,
            source: code,
            original_file_names: sourcemap_meta.original_file_names,
            names: sourcemap_meta.names,
          })));
        }
      }
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

  #[tracing::instrument(level = "debug", skip_all)]
  async fn instantiate_chunks(
    &self,
    chunk_graph: &ChunkGraph,
    ast_table: IndexEcmaAst,
    diagnostics: &mut Diagnostics,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    mut inline_renders: FxHashMap<ChunkIdx, Vec<CarriedRender>>,
  ) -> BuildResult<(IndexInstantiatedChunks, IndexChunkToInstance)> {
    let mut index_chunk_to_instance: IndexChunkToInstance =
      index_vec![None; chunk_graph.chunk_table.len()];
    let mut index_instantiated_chunks: IndexInstantiatedChunks =
      IndexVec::with_capacity(chunk_graph.chunk_table.len());
    let mut ast_table = ast_table;
    let group_bodies_by_chunk = self.print_deferred_group_bodies(chunk_graph, &mut ast_table);
    let chunk_index_to_codegen_rets = self.create_chunk_to_codegen_ret_map(chunk_graph, ast_table);
    let render_export_items_index_vec = &chunk_graph
      .chunk_table
      .chunks
      .iter()
      .map(|item| {
        let mut map: FxIndexMap<SymbolRef, Vec<CompactStr>> = FxIndexMap::default();
        get_export_items(item).into_iter().for_each(|(k, v)| {
          map.entry(v).or_default().push(k);
        });
        map
      })
      .collect();

    try_join_all(
      chunk_index_to_codegen_rets
        .into_iter()
        .zip(group_bodies_by_chunk)
        .enumerate()
        .filter_map(|(idx, (module_id_to_codegen_ret, group_body_codegen_rets))| {
          let chunk_idx =
            ChunkIdx::from_raw(u32::try_from(idx).expect("chunk index should fit in u32"));
          // A record is no file; its modules are printed by the files that carry it.
          if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
            || self.inline_state.is_record(chunk_idx)
          {
            return None;
          }
          let chunk = chunk_graph.chunk_table.get(chunk_idx)?;
          let carried_renders = inline_renders.remove(&chunk_idx).unwrap_or_default();
          Some((
            chunk_idx,
            chunk,
            module_id_to_codegen_ret,
            group_body_codegen_rets,
            carried_renders,
          ))
        })
        .map(
          |(
            chunk_idx,
            chunk,
            module_id_to_codegen_ret,
            group_body_codegen_rets,
            carried_renders,
          )| {
            let resolved_paths = self.resolved_paths.as_ref();
            async move {
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
                group_body_codegen_rets,
                render_export_items_index_vec,
                resolved_paths,
                inline_state: &self.inline_state,
                carried_renders,
              };
              EcmaGenerator::instantiate_chunk(&mut ecma_ctx).await
            }
          },
        ),
    )
    .await?
    .into_iter()
    .for_each(|result| match result {
      Ok(generate_output) => {
        let base_chunk_idx = generate_output.chunk.originate_from;
        let ins_chunk_idx = index_instantiated_chunks.push(generate_output.chunk);
        index_chunk_to_instance[base_chunk_idx] = Some(ins_chunk_idx);
        diagnostics.extend(generate_output.warnings);
      }
      Err(e) => diagnostics.extend(e.into_vec()),
    });

    Ok((index_instantiated_chunks, index_chunk_to_instance))
  }

  /// Prints the body of each root and member of a deferred group on its own, and removes it from
  /// the AST of the module (`render_normal_module_group_body`). The AST keeps the top-level
  /// declarations, and `create_chunk_to_codegen_ret_map` prints them. Returns the printed bodies
  /// of each chunk.
  /// See internal-docs/import-defer/implementation.md ("Rendering").
  fn print_deferred_group_bodies(
    &self,
    chunk_graph: &ChunkGraph,
    ast_table: &mut IndexEcmaAst,
  ) -> Vec<FxHashMap<ModuleIdx, ModuleRenderOutput>> {
    let mut bodies_by_chunk =
      (0..chunk_graph.chunk_table.len()).map(|_| FxHashMap::default()).collect::<Vec<_>>();
    // Only `group_deferred_modules` makes groups. A build with no groups does not read the
    // modules here.
    if chunk_graph.chunk_table.iter().all(|chunk| chunk.module_groups.is_empty()) {
      return bodies_by_chunk;
    }
    let body_starts = chunk_graph
      .chunk_table
      .iter()
      .flat_map(|chunk| &chunk.module_idx_to_render_concatenated_module)
      .filter_map(|(&module_idx, parts)| Some((module_idx, parts.body_start?)))
      .collect::<FxHashMap<_, _>>();
    if body_starts.is_empty() {
      return bodies_by_chunk;
    }
    let bodies = ast_table
      .par_iter_mut_enumerated()
      .filter_map(|(module_idx, ast)| {
        let body_start = *body_starts.get(&module_idx)?;
        let module = self.link_output.module_table[module_idx].as_normal()?;
        let body = crate::utils::render_ecma_module::render_normal_module_group_body(
          module,
          self.options,
          ast.as_mut()?,
          body_start,
        );
        Some((module_idx, body))
      })
      .collect::<Vec<_>>();
    for (module_idx, body) in bodies {
      let chunk_idx = chunk_graph.module_to_chunk[module_idx]
        .expect("a module of a deferred group is in a chunk");
      bodies_by_chunk[chunk_idx.index()].insert(module_idx, body);
    }
    bodies_by_chunk
  }

  /// Create a IndexVecMap from chunk index to related modules codegen return list.
  /// e.g.
  /// modules of chunk1: [ecma1, ecma2, external1]
  /// modules of chunk2: [ecma3, external2]
  /// ret: [
  ///   [Some(ecma1_codegen), Some(ecma2_codegen), None],
  ///   [Some(ecma3_codegen), None],
  /// ]
  ///
  /// This is the last reader of `ast_table`. Each module's AST moves into the work list of the
  /// chunk that prints it and is dropped as soon as that module is printed, so the per-module
  /// arenas are released while the other chunks are still rendering. Holding every AST until
  /// the last chunk is printed would stack the whole AST table on top of all the codegen output.
  /// A module printed by more than one chunk keeps its AST in `ast_table` until the end.
  fn create_chunk_to_codegen_ret_map(
    &self,
    chunk_graph: &ChunkGraph,
    mut ast_table: IndexEcmaAst,
  ) -> Vec<Vec<Option<ModuleRenderOutput>>> {
    let needs_extra_indent = matches!(
      self.options.format,
      rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
    );
    let mut print_count: IndexVec<ModuleIdx, u32> = index_vec![0; ast_table.len()];
    for (chunk_idx, item) in chunk_graph.chunk_table.iter_enumerated() {
      // A record's modules were finalized and printed per carrier; its own ASTs stay untouched.
      if self.inline_state.is_record(chunk_idx) {
        continue;
      }
      for &module_idx in &item.modules {
        print_count[module_idx] += 1;
      }
    }
    let work = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, item)| {
        if self.inline_state.is_record(chunk_idx) {
          return vec![];
        }
        item
          .modules
          .iter()
          .map(|&module_idx| {
            let owned_ast =
              if print_count[module_idx] == 1 { ast_table[module_idx].take() } else { None };
            (module_idx, owned_ast)
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    let shared_ast_table = &ast_table;
    work
      .into_par_iter()
      .map(|modules| {
        modules
          .into_par_iter()
          .map(|(module_idx, owned_ast)| {
            let module = self.link_output.module_table[module_idx].as_normal()?;
            let ast = owned_ast
              .as_ref()
              .or(shared_ast_table[module_idx].as_ref())
              .expect("should have ast");
            // For a root or a member of a deferred group, the AST has only its declarations
            // here (`print_deferred_group_bodies`).
            let initial_indent = u32::from(needs_extra_indent);
            Some(crate::utils::render_ecma_module::render_normal_module(
              module,
              self.options,
              ast,
              initial_indent,
            ))
            // `owned_ast` is dropped here, releasing this module's arena.
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>()
  }

  fn trace_action_assets_ready(index_assets: &AssetVec) {
    if trace_action_enabled!() {
      let mut assets = vec![];
      for asset in index_assets {
        assets.push(action::Asset {
          chunk_id: asset.originate_from.map(ChunkIdx::raw),
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
    .chunk_table
    .chunks
    .iter_enumerated()
    .filter_map(|(idx, chunk)| {
      chunk_graph.chunk_idx_to_reference_ids.get(&idx).map(|reference_ids| {
        reference_ids.iter().map(|reference_id| EmittedChunkInfo {
          reference_id: reference_id.clone(),
          filename: chunk
            .preliminary_filename
            .as_ref()
            .expect("Emitted chunk should have filename")
            .deref()
            .clone(),
        })
      })
    })
    .flatten();
  file_emitter.set_emitted_chunk_info(emitted_chunk_info);
}

fn set_emitted_chunk_filenames(
  file_emitter: &SharedFileEmitter,
  assets: &AssetVec,
  chunk_graph: &ChunkGraph,
) {
  let emitted_chunk_info = assets
    .iter()
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
