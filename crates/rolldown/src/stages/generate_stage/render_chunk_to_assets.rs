use std::{ops::Deref, pin::Pin, sync::Arc};

use futures::future::try_join_all;
use oxc_index::{IndexVec, index_vec};
use oxc_str::CompactStr;
use rolldown_common::{
  Asset, ChunkIdx, EmittedChunkInfo, InstantiationKind, ModuleRenderArgs, ModuleRenderOutput,
  Output, OutputAsset, OutputChunk, SharedFileEmitter, SymbolRef, UsedSymbolRefs,
};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult, Diagnostics};
use rolldown_utils::{
  index_vec_ext::IndexVecRefExt as _,
  indexmap::{FxIndexMap, FxIndexSet},
  poc_prof::{Phase, Timer},
  rayon::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  BundleOutput,
  chunk_graph::ChunkGraph,
  ecmascript::ecma_generator::EcmaGenerator,
  type_alias::{AssetVec, IndexChunkToInstances, IndexEcmaAst, IndexInstantiatedChunks},
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{
    augment_chunk_hash::augment_chunk_hash,
    chunk::{finalize_chunks::finalize_assets, render_chunk_exports::get_export_items},
    render_chunks::render_chunks,
  },
};

use super::{
  CarriedRender, GenerateStage,
  chunk_ast_dce::{self},
};

type ChunkGeneratorFuture<'a> = Pin<
  Box<
    dyn Future<Output = Result<Result<GenerateOutput, BatchedBuildDiagnostic>, anyhow::Error>>
      + 'a
      + Send,
  >,
>;

struct InstantiatedChunks {
  chunks: IndexInstantiatedChunks,
  instances: IndexChunkToInstances,
  optimized: FxHashSet<ChunkIdx>,
  property_cache: Option<oxc::minifier::ManglePropertyCache>,
  pending: FxHashMap<ChunkIdx, chunk_ast_dce::PendingChunk>,
  rendered_text: FxHashSet<ChunkIdx>,
}

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
    // (the last reader), where it is dropped at scope exit by the compiler —
    // releasing the per-module bumpalo arenas before `minify_chunks` and
    // `finalize_assets` allocate.
    let InstantiatedChunks {
      chunks: mut instantiated_chunks,
      instances: index_chunk_to_instances,
      optimized: ast_dce_chunks,
      property_cache: ast_property_cache,
      pending: mut pending_ast,
      rendered_text,
    } = self
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

    let (render_warnings, text_replaced) = render_chunks(
      self.plugin_driver,
      &mut instantiated_chunks,
      self.options,
      &chunk_graph.chunk_table,
      &self.link_output.module_table,
    )
    .await?;
    diagnostics.extend(render_warnings);
    let replaced = pending_ast.keys().filter(|idx| text_replaced.contains(idx)).copied().collect();
    pending_ast.retain(|idx, _| !text_replaced.contains(idx));

    augment_chunk_hash(self.plugin_driver, &mut instantiated_chunks).await?;

    let mangle_cache = Self::minify_chunks(
      self.options,
      &mut instantiated_chunks,
      &ast_dce_chunks,
      ast_property_cache,
      chunk_ast_dce::DeferredChunks { pending: pending_ast, replaced, rendered_text },
      &self.link_output.module_table,
    )?;

    Self::post_banner_footer(&mut instantiated_chunks)?;

    let assets = finalize_assets(
      chunk_graph,
      self.link_output,
      instantiated_chunks,
      &index_chunk_to_instances,
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
            filename: filename.clone(),
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
            filename: filename.clone(),
            source: code,
            original_file_names: sourcemap_meta.original_file_names,
            names: sourcemap_meta.names,
          })));
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
  ) -> BuildResult<InstantiatedChunks> {
    let mut index_chunk_to_instances: IndexChunkToInstances =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];
    let mut index_instantiated_chunks: IndexInstantiatedChunks =
      IndexVec::with_capacity(chunk_graph.chunk_table.len());
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
    let (chunk_index_to_codegen_rets, mut prepared_chunks) = self.create_chunk_to_codegen_ret_map(
      chunk_graph,
      ast_table,
      used_symbol_refs,
      order_state,
      render_export_items_index_vec,
      &mut inline_renders,
    );
    let ast_dce_chunks: FxHashSet<ChunkIdx> = prepared_chunks.keys().copied().collect();
    let mut property_cache = None;
    let mut pending_ast = FxHashMap::default();
    let mut rendered_text = FxHashSet::default();

    try_join_all(
      chunk_index_to_codegen_rets
        .into_iter()
        .enumerate()
        .filter_map(|(idx, module_id_to_codegen_ret)| {
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
          let chunk_ast_dce = prepared_chunks.remove(&chunk_idx);
          Some((chunk_idx, chunk, module_id_to_codegen_ret, carried_renders, chunk_ast_dce))
        })
        .flat_map(
          |(chunk_idx, chunk, module_id_to_codegen_ret, carried_renders, chunk_ast_dce)| {
            let resolved_paths = self.resolved_paths.as_ref();
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
                render_export_items_index_vec,
                resolved_paths,
                inline_state: &self.inline_state,
                carried_renders,
                chunk_ast_dce: None,
                defer_ast: false,
              };
              let mut pending = None;
              let mut wrapped = None;
              if let Some(prepared) = chunk_ast_dce {
                if matches!(
                  self.options.format,
                  rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
                ) || self.options.is_dev_mode_enabled()
                  || self.inline_state.has_records()
                  || chunk_ast_dce::complete_program(self.options, self.plugin_driver)
                {
                  wrapped = Some(prepared);
                  ecma_ctx.defer_ast = true;
                } else {
                  (ecma_ctx.chunk_ast_dce, pending) = prepared.into_render_state(&ecma_ctx);
                  ecma_ctx.defer_ast = pending.is_some();
                }
              }
              let mut result = EcmaGenerator::instantiate_chunk(&mut ecma_ctx).await?;
              if let Ok(output) = &mut result {
                if let Some(prepared) = wrapped {
                  if chunk_ast_dce::complete_program(self.options, self.plugin_driver) {
                    // Arbitrary addon fragments can change tokenization and statement boundaries
                    // across module text. Parse their actual rendered input after hooks, using the
                    // shared optimizer. Empty-statement skeleton slots cannot preserve this contract.
                    // See internal-docs/chunk-ast-minify/implementation.md.
                    output.minify_rendered_text = true;
                  } else {
                    pending = match prepared.into_wrapped(
                      &ecma_ctx,
                      output.wrapped_plan.take().expect("wrapped format plan"),
                    ) {
                      Ok(pending) => Some(pending),
                      Err(errors) => return Ok(Err(errors)),
                    };
                  }
                } else if let Some(owner) = &mut pending {
                  let (prefix, suffix) =
                    output.envelope.take().expect("deferred rendering envelope");
                  owner.set_envelope(prefix, suffix);
                }
                output.pending_ast = pending;
              }
              Ok(result)
            });
            [ecma_chunks_future]
          },
        ),
    )
    .await?
    .into_iter()
    .for_each(|result| match result {
      Ok(mut generate_output) => {
        if generate_output.minify_rendered_text {
          rendered_text.insert(generate_output.chunks[0].originate_from);
        }
        if let Some(pending) = generate_output.pending_ast.take() {
          let idx = generate_output.chunks[0].originate_from;
          pending_ast.insert(idx, pending);
        }
        if let Some(cache) = generate_output.property_cache.take() {
          if property_cache.is_some() {
            diagnostics.extend([rolldown_error::BuildDiagnostic::invalid_option(
              rolldown_error::InvalidOptionType::ManglePropertiesWithMultipleChunks,
            )]);
          } else {
            property_cache = Some(cache);
          }
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
      instances.sort_by_cached_key(|ins_chunk_idx| {
        index_instantiated_chunks[*ins_chunk_idx].preliminary_filename.as_str()
      });
    });

    Ok(InstantiatedChunks {
      chunks: index_instantiated_chunks,
      instances: index_chunk_to_instances,
      optimized: ast_dce_chunks,
      property_cache,
      pending: pending_ast,
      rendered_text,
    })
  }

  /// Create a IndexVecMap from chunk index to related modules codegen return list.
  /// e.g.
  /// modules of chunk1: [ecma1, ecma2, external1]
  /// modules of chunk2: [ecma3, external2]
  /// ret: [
  ///   [Some(ecma1_codegen), Some(ecma2_codegen), None],
  ///   [Some(ecma3_codegen), None],
  /// ]
  // This is the last reader of finalized ASTs. Eligible chunks consume their ASTs into a private
  // owner that retains every arena during preparation, parallel printing, and unwind.
  fn create_chunk_to_codegen_ret_map(
    &self,
    chunk_graph: &ChunkGraph,
    mut ast_table: IndexEcmaAst,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    render_export_items_index_vec: &IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>>,
    inline_renders: &mut FxHashMap<ChunkIdx, Vec<CarriedRender>>,
  ) -> (Vec<Vec<Option<ModuleRenderOutput>>>, FxHashMap<ChunkIdx, chunk_ast_dce::PreparedChunk>) {
    let prepared_chunks = self.run_chunk_ast_dce(
      chunk_graph,
      &mut ast_table,
      used_symbol_refs,
      order_state,
      render_export_items_index_vec,
      inline_renders,
    );
    let needs_extra_indent = matches!(
      self.options.format,
      rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
    );
    let ret = chunk_graph
      .chunk_table
      .par_iter_enumerated()
      .map(|(chunk_idx, item)| {
        // A record's modules were finalized and printed per carrier; its own ASTs stay untouched.
        if self.inline_state.is_record(chunk_idx) {
          return vec![];
        }
        let prepared = prepared_chunks.get(&chunk_idx);
        // Positions of the chunk's normal modules, as `chunk_ast_dce` numbers them.
        let positions: FxHashMap<rolldown_common::ModuleIdx, usize> = if prepared.is_some() {
          item
            .modules
            .iter()
            .filter(|idx| self.link_output.module_table[**idx].is_normal())
            .enumerate()
            .map(|(pos, idx)| (*idx, pos))
            .collect()
        } else {
          FxHashMap::default()
        };
        item
          .modules
          .par_iter()
          .map(|&module_idx| match self.link_output.module_table[module_idx].as_normal() {
            Some(module) => {
              #[expect(clippy::bool_to_int_with_if)]
              let initial_indent = if needs_extra_indent { 1 } else { 0 };
              let t = Timer::start();
              let out = match prepared {
                Some(prepared) => prepared.print_module(
                  positions[&module_idx],
                  module,
                  self.options,
                  initial_indent,
                  &self.link_output.module_table,
                ),
                None => module.render_with(
                  self.options,
                  &ModuleRenderArgs::Ecma {
                    ast: ast_table[module.idx].as_ref().expect("should have ast"),
                  },
                  initial_indent,
                  true,
                ),
              };
              t.stop(Phase::ModulePrint);
              if let Some(dir) = std::env::var_os("ROLLDOWN_CHUNK_AST_DCE_DUMP") {
                let name = module.stable_id.as_str().replace(['/', '\\', '\0'], "_");
                std::fs::write(
                  std::path::Path::new(&dir).join(format!("module.{name}.js")),
                  &out.code,
                )
                .unwrap();
              }
              Some(out)
            }
            _ => None,
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    drop(ast_table);
    (ret, prepared_chunks)
  }

  /// PoC: runs `chunk_ast_dce::dce_chunk` on every eligible chunk, in parallel across chunks.
  fn run_chunk_ast_dce(
    &self,
    chunk_graph: &ChunkGraph,
    ast_table: &mut IndexEcmaAst,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    render_export_items_index_vec: &IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>>,
    inline_renders: &mut FxHashMap<ChunkIdx, Vec<CarriedRender>>,
  ) -> FxHashMap<ChunkIdx, chunk_ast_dce::PreparedChunk> {
    let Some(minify_options) = chunk_ast_dce::build_minify_options(self.options) else {
      return FxHashMap::default();
    };
    let make_ctx = |chunk_idx: ChunkIdx| GenerateContext {
      chunk_idx,
      chunk: &chunk_graph.chunk_table[chunk_idx],
      options: self.options,
      link_output: self.link_output,
      used_symbol_refs,
      order_wrap_state: order_state,
      chunk_graph,
      plugin_driver: self.plugin_driver,
      module_id_to_codegen_ret: vec![],
      render_export_items_index_vec,
      resolved_paths: self.resolved_paths.as_ref(),
      inline_state: &self.inline_state,
      carried_renders: vec![],
      chunk_ast_dce: None,
      defer_ast: false,
    };
    let mut work = vec![];
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
        || self.inline_state.is_record(chunk_idx)
      {
        #[cfg(feature = "testing")]
        chunk_ast_dce::record_coverage(
          self.options,
          "chunk",
          "post_chunk_operation_or_inline_record",
          chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
        );
        continue;
      }
      let mut asts = Vec::with_capacity(chunk.modules.len());
      for module_idx in &chunk.modules {
        if self.link_output.module_table[*module_idx].as_normal().is_some() {
          asts.push(ast_table[*module_idx].as_ref().expect("should have ast"));
        }
      }
      if let Some(carried) = inline_renders.get(&chunk_idx) {
        asts.extend(
          carried
            .iter()
            .flat_map(|record| record.modules.iter().filter_map(|module| module.ast.as_ref())),
        );
      }
      if chunk_ast_dce::chunk_eligible(&make_ctx(chunk_idx)) {
        let mut owned = chunk_ast_dce::ChunkAst::new(asts.len());
        for module_idx in &chunk.modules {
          if self.link_output.module_table[*module_idx].as_normal().is_some() {
            owned.push(ast_table[*module_idx].take().expect("should have ast"));
          }
        }
        if chunk_ast_dce::retain_inline_asts(self.options, self.plugin_driver)
          && let Some(carried) = inline_renders.get_mut(&chunk_idx)
        {
          owned.push_inline(carried);
        }
        work.push((chunk_idx, owned));
      }
    }
    work
      .into_par_iter()
      .map(|(chunk_idx, asts)| (chunk_idx, asts.prepare(&make_ctx(chunk_idx), minify_options)))
      .collect()
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
