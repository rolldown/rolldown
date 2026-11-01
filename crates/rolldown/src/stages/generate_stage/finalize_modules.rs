use std::sync::Arc;

use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ConcatenateWrappedModuleKind, ModuleIdx, ModuleRenderArgs, ModuleRenderOutput,
  NormalModule, PrependRenderedImport, UsedSymbolRefs,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintCommentsOptions, PrintOptions};
use rolldown_error::{BuildResult, Severity};
use rolldown_utils::{
  index_vec_ext::{IndexVecExt as _, IndexVecRefExt as _},
  rayon::ParallelIterator as _,
};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::debug_span;

use crate::{chunk_graph::ChunkGraph, module_finalizers::ScopeHoistingFinalizerContext};

use super::{
  FinalEsmInitMetadata, GenerateStage, Sealed,
  inline_common_chunks::{BridgeCallees, bare_bridge_callee},
  resolve_file_urls::ResolvedFileUrls,
};

impl GenerateStage<'_> {
  pub(crate) fn render_normal_module_with(
    options: &rolldown_common::NormalizedBundlerOptions,
    link_output: &crate::stages::link_stage::LinkStageOutput,
    module: &NormalModule,
    ast: &EcmaAst,
  ) -> ModuleRenderOutput {
    let needs_extra_indent = matches!(
      options.format,
      rolldown_common::OutputFormat::Iife | rolldown_common::OutputFormat::Umd
    );
    #[expect(clippy::bool_to_int_with_if)]
    let initial_indent = if needs_extra_indent
      || !matches!(
        link_output.metas[module.idx].concatenated_wrapped_module_kind,
        ConcatenateWrappedModuleKind::None
      ) {
      1
    } else {
      0
    };
    // `NormalModule::render` maps no virtual module, but a native cell's load map
    // points at its donor's authored file; print it as `render` would, mapped.
    if options.sourcemap.is_some()
      && module.ecma_view.mutations.is_empty()
      && crate::utils::native_records::is_native_cell(&module.id)
    {
      let mut comments: PrintCommentsOptions = options.comments.into();
      comments.annotation = true;
      let output = EcmaCompiler::print_with(
        ast,
        PrintOptions {
          sourcemap: true,
          filename: module.id.to_string(),
          comments,
          initial_indent,
          ascii_only: options.code_splitting.is_disabled(),
        },
      );
      return ModuleRenderOutput {
        code: output.code,
        map: output.map.map(|map| map.into_owned()),
      };
    }
    module.render(options, &ModuleRenderArgs::Ecma { ast }, initial_indent)
  }

  pub(super) fn render_normal_module(
    &self,
    module: &NormalModule,
    ast: &EcmaAst,
  ) -> ModuleRenderOutput {
    Self::render_normal_module_with(self.options, self.link_output, module, ast)
  }

  pub(crate) fn finalize_and_render_cached_module(
    ctx: &crate::types::generator::GenerateContext<'_>,
    module: &NormalModule,
  ) -> Option<std::sync::Arc<[Box<dyn rolldown_sourcemap::Source + Send + Sync>]>> {
    let ast_table = ctx.ast_table?;
    let final_esm_init_metadata = ctx.final_esm_init_metadata?;
    let mut ast = ast_table.get(module.idx)?.as_ref()?.clone_with_another_arena();
    let ast_scope = &ctx.link_output.symbol_db[module.idx].as_ref()?.ast_scopes;
    let empty_file_urls = rustc_hash::FxHashMap::default();
    let empty_inline_state = super::InlineCommonChunksState::default();
    let has_enum_inlining = ctx.link_output.has_enum_inlining;
    let finalizer_ctx = ScopeHoistingFinalizerContext {
      idx: module.idx,
      chunk: ctx.chunk,
      chunk_idx: ctx.chunk_idx,
      file_idx: ctx.chunk_idx,
      chunk_graph: ctx.chunk_graph,
      symbol_db: &ctx.link_output.symbol_db,
      linking_info: &ctx.link_output.metas[module.idx],
      module,
      stmt_infos: &ctx.link_output.stmt_infos[module.idx],
      index_stmt_infos: &ctx.link_output.stmt_infos,
      modules: &ctx.link_output.module_table.modules,
      linking_infos: &ctx.link_output.metas,
      dynamic_import_usage: ctx.link_output.dynamic_import_exports_usage_map.get(&module.idx),
      order_wrap_state: ctx.order_wrap_state,
      final_esm_init_metadata,
      used_symbol_refs: ctx.used_symbol_refs,
      runtime: &ctx.link_output.runtime,
      options: ctx.options,
      file_emitter: &ctx.plugin_driver.file_emitter,
      constant_value_map: &ctx.link_output.global_constant_symbol_map,
      safely_merge_cjs_ns_map: &ctx.link_output.safely_merge_cjs_ns_map,
      retained_export_symbols: &ctx.link_output.retained_export_symbols,
      resolved_paths: ctx.resolved_paths,
      resolved_file_urls: &empty_file_urls,
      has_enum_inlining,
      unreachable_modules: &ctx.link_output.unreachable_modules,
      inline_state: &empty_inline_state,
    };
    let _ = finalizer_ctx.finalize_normal_module(&mut ast, ast_scope);
    let rendered = Self::render_normal_module_with(
      ctx.options,
      ctx.link_output,
      module,
      &ast,
    );
    drop(ast);
    crate::utils::render_ecma_module::render_ecma_module(module, ctx.options, rendered).sources
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn finalize_modules(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    resolved_file_urls: &ResolvedFileUrls,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
    reused_chunks: &FxHashSet<ChunkIdx>,
  ) -> BuildResult<IndexVec<ModuleIdx, Option<ModuleRenderOutput>>> {
    let has_enum_inlining = self.link_output.has_enum_inlining;
    let has_required_order_runtime = !order_state.required_runtime_helpers().is_empty();
    let bridge_callees: FxHashMap<ChunkIdx, BridgeCallees> =
      if self.inline_state.reading_files().is_empty() {
        FxHashMap::default()
      } else {
        let wrapper_refs = self.wrapper_refs(order_state);
        self
          .inline_state
          .reading_files()
          .iter()
          .map(|&file| {
            let callees = self.inline_state.bridge_callees(
              file,
              chunk_graph,
              &wrapper_refs,
              &self.link_output.symbol_db,
            );
            (file, callees)
          })
          .collect()
      };
    // Off-strict, lowering never mutates the chunk graph, so the liveness guard cannot fire.
    let strict = self.options.is_strict_execution_order_enabled();

    let finalized = debug_span!("finalize_modules").in_scope(|| {
      if self.cache.is_none() && !self.inline_state.has_records() {
        let mut ast_table = std::mem::take(&mut self.ast_table);
        let result = ast_table
          .par_iter_mut_enumerated()
          .filter(|(idx, _ast)| {
            self.link_output.module_table[*idx].as_normal().is_some_and(|m| {
              let is_required_order_runtime =
                m.idx == self.link_output.runtime.id() && has_required_order_runtime;
              (self.link_output.metas[m.idx].is_included || is_required_order_runtime)
                && (!strict || chunk_graph.module_is_in_live_chunk(*idx))
                && chunk_graph.module_to_chunk[*idx]
                  .is_none_or(|chunk_idx| !reused_chunks.contains(&chunk_idx))
            })
          })
          .filter_map(|(idx, ast_slot)| {
            let mut ast = ast_slot.take()?;
            let module = self.link_output.module_table[idx].as_normal().unwrap();
            let ast_scope = &self.link_output.symbol_db[idx].as_ref().unwrap().ast_scopes;
            let chunk_idx = chunk_graph.module_to_chunk[idx].unwrap();
            let chunk = &chunk_graph.chunk_table[chunk_idx];
            let linking_info = &self.link_output.metas[module.idx];
            let ctx = ScopeHoistingFinalizerContext {
              idx,
              chunk,
              chunk_idx,
              file_idx: chunk_idx,
              chunk_graph,
              symbol_db: &self.link_output.symbol_db,
              linking_info,
              module,
              stmt_infos: &self.link_output.stmt_infos[idx],
              index_stmt_infos: &self.link_output.stmt_infos,
              modules: &self.link_output.module_table.modules,
              linking_infos: &self.link_output.metas,
              dynamic_import_usage: self.link_output.dynamic_import_exports_usage_map.get(&idx),
              order_wrap_state: order_state,
              final_esm_init_metadata,
              used_symbol_refs,
              runtime: &self.link_output.runtime,
              options: self.options,
              file_emitter: &self.plugin_driver.file_emitter,
              constant_value_map: &self.link_output.global_constant_symbol_map,
              safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
              retained_export_symbols: &self.link_output.retained_export_symbols,
              resolved_paths: self.resolved_paths.as_ref(),
              resolved_file_urls,
              has_enum_inlining,
              unreachable_modules: &self.link_output.unreachable_modules,
              inline_state: &self.inline_state,
            };

            let concatenated_wrapped_module_kind = ctx.linking_info.concatenated_wrapped_module_kind;
            let (
              transferred_import_record,
              rendered_concatenated_wrapped_module_parts,
              mut module_diagnostics,
            ) = ctx.finalize_normal_module(&mut ast, ast_scope);
            if let Some(callees) = bridge_callees.get(&chunk_idx) {
              module_diagnostics.extend(bare_bridge_callee(
                ast.program(),
                callees,
                &module.stable_id,
              ));
            }

            let has_module_error =
              module_diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error);
            let (rendered, deferred_ast) = if has_module_error {
              (None, None)
            } else if chunk.insert_map.contains_key(&idx) {
              (None, Some(ast))
            } else {
              let rendered = self.render_normal_module(module, &ast);
              drop(ast);
              (Some(rendered), None)
            };

            let payload = (!transferred_import_record.is_empty()
              || !matches!(concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None))
            .then_some((idx, transferred_import_record, rendered_concatenated_wrapped_module_parts));
            Some((idx, payload, module_diagnostics, rendered, deferred_ast, None))
          })
          .collect::<Vec<_>>();
        crate::utils::defer_drop::spawn_drop(ast_table);
        return result;
      }
      let can_cache_finalized = self.cache.is_some()
        && !self.inline_state.has_records()
        && self.resolved_paths.is_none()
        && !self.options.is_dev_mode_enabled();
      let runtime_id = self.link_output.runtime.id();
      let chunk_runtime_digests: IndexVec<ChunkIdx, [u128; 2]> = if can_cache_finalized {
        use std::hash::Hash;
        use xxhash_rust::xxh3::Xxh3;
        chunk_graph
          .chunk_table
          .chunks
          .par_iter_enumerated()
          .map(|(cidx, chunk)| {
            if chunk_graph.post_chunk_optimization_operations.contains_key(&cidx) {
              return [0, 0];
            }
            let mut h0 = Xxh3::with_seed(0x243f6a8885a308d3);
            let mut h1 = Xxh3::with_seed(0x13198a2e03707344);
            if let Some(db) = self.link_output.symbol_db[runtime_id].as_ref() {
              for sym_id in db.classic_data.indices() {
                let sref: rolldown_common::SymbolRef = (runtime_id, sym_id).into();
                let cref = self.link_output.symbol_db.canonical_ref_for(sref);
                let cname = chunk.canonical_names.get(&cref).map(|n| n.as_str());
                (sym_id.index(), cname).hash(&mut h0);
                (sym_id.index(), cname).hash(&mut h1);
              }
            }
            [h0.digest128(), h1.digest128()]
          })
          .collect::<Vec<_>>()
          .into()
      } else {
        IndexVec::default()
      };
      let importee_reexport_wrap_digest = if can_cache_finalized {
        self.compute_importee_reexport_wrap_digests(
          chunk_graph,
          order_state,
          final_esm_init_metadata,
        )
      } else {
        IndexVec::default()
      };
      self
        .ast_table
        .par_iter_enumerated()
        .filter(|(idx, _ast)| {
          self.link_output.module_table[*idx].as_normal().is_some_and(|m| {
            let is_required_order_runtime =
              m.idx == self.link_output.runtime.id() && has_required_order_runtime;
            (self.link_output.metas[m.idx].is_included || is_required_order_runtime)
              && (!strict || chunk_graph.module_is_in_live_chunk(*idx))
              && chunk_graph.module_to_chunk[*idx].is_none_or(|chunk_idx| {
                !reused_chunks.contains(&chunk_idx) && !self.inline_state.is_record(chunk_idx)
              })
          })
        })
        .filter_map(|(idx, ast)| {
          let module = self.link_output.module_table[idx].as_normal().unwrap();
          let chunk_idx = chunk_graph.module_to_chunk[idx].unwrap();
          let chunk = &chunk_graph.chunk_table[chunk_idx];
          let linking_info = &self.link_output.metas[module.idx];
          let finalize_digest = if can_cache_finalized {
            self.compute_module_finalize_digest(
              idx,
              module,
              chunk_idx,
              chunk,
              linking_info,
              chunk_graph,
              used_symbol_refs,
              order_state,
              final_esm_init_metadata,
              chunk_runtime_digests[chunk_idx],
              &importee_reexport_wrap_digest,
            )
          } else {
            None
          };
          if let Some(digest) = finalize_digest
            && let Some(cached) = self
              .cache
              .as_ref()
              .and_then(|c| c.cached_finalized_modules.get(&idx))
            && cached.digest == digest
          {
            return Some((
              idx,
              None,
              Vec::new(),
              None,
              None,
              Some((digest, cached.sources.0.clone(), true, true)),
            ));
          }
          let mut ast = ast.as_ref()?.clone_with_another_arena();
          let ast_scope = &self.link_output.symbol_db[idx].as_ref().unwrap().ast_scopes;
          let ctx = ScopeHoistingFinalizerContext {
            idx,
            chunk,
            chunk_idx,
            file_idx: chunk_idx,
            chunk_graph,
            symbol_db: &self.link_output.symbol_db,
            linking_info,
            module,
            stmt_infos: &self.link_output.stmt_infos[idx],
            index_stmt_infos: &self.link_output.stmt_infos,
            modules: &self.link_output.module_table.modules,
            linking_infos: &self.link_output.metas,
            dynamic_import_usage: self.link_output.dynamic_import_exports_usage_map.get(&idx),
            order_wrap_state: order_state,
            final_esm_init_metadata,
            used_symbol_refs,
            runtime: &self.link_output.runtime,
            options: self.options,
            file_emitter: &self.plugin_driver.file_emitter,
            constant_value_map: &self.link_output.global_constant_symbol_map,
            safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
            retained_export_symbols: &self.link_output.retained_export_symbols,
            resolved_paths: self.resolved_paths.as_ref(),
            resolved_file_urls,
            has_enum_inlining,
            unreachable_modules: &self.link_output.unreachable_modules,
            inline_state: &self.inline_state,
          };

          let concatenated_wrapped_module_kind = ctx.linking_info.concatenated_wrapped_module_kind;
          let (
            transferred_import_record,
            rendered_concatenated_wrapped_module_parts,
            mut module_diagnostics,
          ) = ctx.finalize_normal_module(&mut ast, ast_scope);
          if let Some(callees) = bridge_callees.get(&chunk_idx) {
            module_diagnostics.extend(bare_bridge_callee(
              ast.program(),
              callees,
              &module.stable_id,
            ));
          }

          let has_module_error =
            module_diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error);
          let payload = (!transferred_import_record.is_empty()
            || !matches!(concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None))
          .then_some((idx, transferred_import_record, rendered_concatenated_wrapped_module_parts));
          if !has_module_error
            && !chunk.insert_map.contains_key(&idx)
            && payload.is_none()
            && let Some(digest) = finalize_digest
          {
            let rendered = self.render_normal_module(module, &ast);
            drop(ast);
            let ecma = crate::utils::render_ecma_module::render_ecma_module(
              module,
              self.options,
              rendered,
            );
            let cacheable = module_diagnostics.is_empty() && ecma.warnings.is_empty();
            module_diagnostics.extend(ecma.warnings);
            return Some((
              idx,
              None,
              module_diagnostics,
              None,
              None,
              Some((digest, ecma.sources, cacheable, false)),
            ));
          }
          let (rendered, deferred_ast) = if has_module_error {
            (None, None)
          } else if chunk.insert_map.contains_key(&idx) {
            (None, Some(ast))
          } else {
            let rendered = self.render_normal_module(module, &ast);
            drop(ast);
            (Some(rendered), None)
          };

          Some((idx, payload, module_diagnostics, rendered, deferred_ast, None))
        })
        .collect::<Vec<_>>()
    });

    if let Some(cache) = self.cache.as_deref_mut() {
      let modules_len = self.link_output.module_table.modules.len();
      cache.cached_finalized_modules.retain(|idx, _| {
        idx.index() < modules_len && self.link_output.module_table[*idx].is_normal()
      });
      cache.current_rendered_sources.clear();
      cache.current_module_finalize_digests.clear();
      cache.current_module_was_hit.clear();
    }
    let mut normalized_transfer_parts_rendered_maps = FxHashMap::default();
    let mut diagnostics = vec![];
    let mut rendered_modules: IndexVec<ModuleIdx, Option<ModuleRenderOutput>> = IndexVec::from_vec(
      (0..self.link_output.module_table.modules.len()).map(|_| None).collect(),
    );
    let mut deferred_asts: Vec<(ModuleIdx, EcmaAst)> = Vec::new();
    for (idx, payload, module_diagnostics, rendered, deferred_ast, cached_entry) in finalized {
      diagnostics.extend(module_diagnostics);
      if let Some((digest, sources, cacheable, was_hit)) = cached_entry
        && let Some(cache) = self.cache.as_deref_mut()
      {
        let wrapped = crate::types::scan_stage_cache::CachedModuleSources(sources);
        if cacheable {
          if !was_hit {
            cache.cached_finalized_modules.insert(
              idx,
              crate::types::scan_stage_cache::CachedFinalizedModule {
                digest,
                sources: crate::types::scan_stage_cache::CachedModuleSources(None),
              },
            );
          }
          cache.current_module_finalize_digests.insert(idx, digest);
          if was_hit {
            cache.current_module_was_hit.insert(idx);
          }
        }
        if wrapped.0.is_some() {
          cache.current_rendered_sources.insert(idx, wrapped);
        }
      }
      if let Some(rendered) = rendered {
        rendered_modules[idx] = Some(rendered);
      }
      if let Some(ast) = deferred_ast {
        deferred_asts.push((idx, ast));
      }
      let Some((idx, transferred_import_record, rendered_concatenated_module_parts)) = payload
      else {
        continue;
      };
      for (rec_idx, rendered_string) in transferred_import_record {
        normalized_transfer_parts_rendered_maps.insert((idx, rec_idx), rendered_string);
      }
      let chunk_idx = chunk_graph.module_to_chunk[idx].expect("should have chunk idx");
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      chunk
        .module_idx_to_render_concatenated_module
        .insert(idx, rendered_concatenated_module_parts);
    }

    let has_error = diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error);
    self.link_output.diagnostics.extend(diagnostics);
    if has_error {
      let errors = self.link_output.diagnostics.extract_errors();
      Err(errors)?;
    }

    let ast_table = std::mem::take(&mut self.ast_table);
    self.finalize_inline_copies(
      chunk_graph,
      &ast_table,
      resolved_file_urls,
      used_symbol_refs,
      order_state,
      final_esm_init_metadata,
    )?;
    self.ast_table = ast_table;
    if self.cache.is_none() {
      let dead_ast = std::mem::take(&mut self.ast_table);
      crate::utils::defer_drop::spawn_drop(dead_ast);
    }

    if !normalized_transfer_parts_rendered_maps.is_empty() {
      for chunk in chunk_graph.chunk_table.iter_mut() {
        for (module_idx, recs) in &chunk.insert_map {
          let Some(module) = self.link_output.module_table[*module_idx].as_normal_mut() else {
            continue;
          };
          for (importer_idx, rec_idx) in recs {
            if let Some(rendered_string) =
              normalized_transfer_parts_rendered_maps.get(&(*importer_idx, *rec_idx))
            {
              module
                .ecma_view
                .mutations
                .push(Arc::new(PrependRenderedImport { intro: rendered_string.clone() }));
            }
          }
        }
      }
    }

    for (idx, ast) in deferred_asts {
      if let Some(module) = self.link_output.module_table[idx].as_normal() {
        rendered_modules[idx] = Some(self.render_normal_module(module, &ast));
      }
    }

    Ok(rendered_modules)
  }

  fn compute_importee_reexport_wrap_digests(
    &self,
    chunk_graph: &ChunkGraph,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
  ) -> IndexVec<ModuleIdx, Option<(usize, (u128, u128))>> {
    use rolldown_utils::index_vec_ext::IndexVecRefExt as _;
    use std::hash::Hash;

    struct RegHasher(u128);
    impl std::hash::Hasher for RegHasher {
      #[inline]
      fn finish(&self) -> u64 {
        (self.0 as u64) ^ ((self.0 >> 64) as u64)
      }
      #[inline]
      fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
          self.0 = (self.0 ^ (b as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
        }
      }
      #[inline]
      fn write_u8(&mut self, i: u8) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
      #[inline]
      fn write_u32(&mut self, i: u32) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
    }
    let sym_key = |s: rolldown_common::SymbolRef| -> (u32, u32) {
      if self.link_output.symbol_db.is_facade_symbol(s) {
        (s.owner.raw(), u32::MAX)
      } else {
        (s.owner.raw(), s.symbol.index() as u32)
      }
    };
    self
      .link_output
      .module_table
      .modules
      .par_iter_enumerated()
      .map(|(importee_idx, m)| {
        m.as_normal()?;
        let importee_meta = &self.link_output.metas[importee_idx];
        let mut fexp_acc = (0u128, 0u128);
        let mut fexp_count = 0usize;
        for exp in importee_meta.resolved_exports.values() {
          let exp_owner = exp.symbol_ref.owner;
          if exp_owner != importee_idx {
            let exp_meta = &self.link_output.metas[exp_owner];
            if !matches!(exp_meta.wrap_kind(), rolldown_common::WrapKind::None)
              || order_state.esm_init_target(exp_owner, exp_meta).is_some()
            {
              let mut h0 = RegHasher(0x243f6a8885a308d3_13198a2e03707344);
              let mut h1 = RegHasher(0xa4093822299f31d0_082efa98ec4e6c89);
              let v = (
                exp_owner.raw(),
                exp_meta.wrap_kind() as u8,
                exp_meta.wrapper_ref.map(&sym_key),
                exp_meta
                  .wrapper_stmt_info
                  .is_some_and(|s| exp_meta.stmt_info_included.has_bit(s)),
                order_state.esm_init_target(exp_owner, exp_meta).map(|t| {
                  (sym_key(t.wrapper_ref), t.tla_tainted, t.origin as u8)
                }),
                final_esm_init_metadata.init_is_noop(exp_owner),
                chunk_graph.module_to_chunk[exp_owner].map(|c| c.raw()),
              );
              v.hash(&mut h0);
              v.hash(&mut h1);
              let d0 = h0.0 ^ (h0.0 >> 63);
              let d1 = h1.0 ^ (h1.0 >> 63);
              fexp_acc = (fexp_acc.0.wrapping_add(d0), fexp_acc.1.wrapping_add(d1));
              fexp_count += 1;
            }
          }
        }
        (fexp_count > 0).then_some((fexp_count, fexp_acc))
      })
      .collect::<Vec<_>>()
      .into()
  }

  #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
  fn compute_module_finalize_digest(
    &self,
    module_idx: ModuleIdx,
    module: &NormalModule,
    chunk_idx: ChunkIdx,
    chunk: &rolldown_common::Chunk,
    meta: &crate::types::linking_metadata::LinkingMetadata,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &super::order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
    runtime_digest: [u128; 2],
    importee_reexport_wrap_digest: &IndexVec<ModuleIdx, Option<(usize, (u128, u128))>>,
  ) -> Option<[u128; 2]> {
    use std::hash::Hash;

    let cache = self.cache.as_ref()?;
    if module_idx == self.link_output.runtime.id()
      || !module.ecma_view.rolldown_file_url_references.is_empty()
      || !module.ecma_view.mutations.is_empty()
      || module.ecma_view.json_module_none_self_reference_included_symbol.is_some()
      || chunk.insert_map.contains_key(&module_idx)
      || chunk.remove_map.contains_key(&module_idx)
      || !meta.shimmed_missing_exports.is_empty()
      || !meta.deferred_imports.is_empty()
      || !matches!(meta.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::None)
      || !order_state.order_cjs_carriers_for_importee(module_idx).is_empty()
    {
      return None;
    }
    let scan_gen = cache.module_scan_generation(module_idx);
    if scan_gen == 0 {
      return None;
    }

    struct RegHasher(u128);
    impl std::hash::Hasher for RegHasher {
      #[inline]
      fn finish(&self) -> u64 {
        (self.0 as u64) ^ ((self.0 >> 64) as u64)
      }
      #[inline]
      fn write(&mut self, bytes: &[u8]) {
        let mut h = self.0;
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
          let k = u64::from_le_bytes(chunk.try_into().unwrap()) as u128;
          h = (h ^ k).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
          h ^= h >> 61;
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
          let mut tail = [0u8; 8];
          tail[..rem.len()].copy_from_slice(rem);
          let k = u64::from_le_bytes(tail) as u128;
          h = (h ^ k ^ ((rem.len() as u128) << 64))
            .wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
          h ^= h >> 61;
        }
        self.0 = h;
      }
      #[inline]
      fn write_u8(&mut self, i: u8) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
      #[inline]
      fn write_u32(&mut self, i: u32) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
      #[inline]
      fn write_u64(&mut self, i: u64) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
      #[inline]
      fn write_usize(&mut self, i: usize) {
        self.0 = (self.0 ^ (i as u128)).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
      }
      #[inline]
      fn write_u128(&mut self, i: u128) {
        let mut h = (self.0 ^ i).wrapping_mul(0x9e3779b97f4a7c15_f39cc0605cedc835);
        h ^= h >> 61;
        self.0 = h;
      }
    }
    struct DualHasher(RegHasher, RegHasher);
    impl DualHasher {
      #[inline]
      fn new() -> Self {
        Self(
          RegHasher(0x243f6a8885a308d3_13198a2e03707344),
          RegHasher(0xa4093822299f31d0_082efa98ec4e6c89),
        )
      }
      #[inline]
      fn write_val<T: Hash + ?Sized>(&mut self, val: &T) {
        val.hash(&mut self.0);
        val.hash(&mut self.1);
      }
      #[inline]
      fn write_unordered(&mut self, tag: u8, len: usize, acc: (u128, u128)) {
        self.write_val(&(tag, len, acc.0, acc.1));
      }
      #[inline]
      fn finish(self) -> [u128; 2] {
        [
          self.0.0 ^ (self.0.0 >> 63),
          self.1.0 ^ (self.1.0 >> 63),
        ]
      }
    }

    let hash_const_export_meta =
      |h: &mut DualHasher, cmeta: Option<&rolldown_common::ConstExportMeta>| match cmeta {
        None => h.write_val(&0u8),
        Some(cmeta) => {
          h.write_val(&(1u8, cmeta.commonjs_export, cmeta.safe_to_inline));
          match &cmeta.value {
            rolldown_common::ConstantValue::Number(n) => h.write_val(&(1u8, n.to_bits())),
            rolldown_common::ConstantValue::BigInt(s) => h.write_val(&(2u8, s)),
            rolldown_common::ConstantValue::String(s) => h.write_val(&(3u8, s.as_str())),
            rolldown_common::ConstantValue::Boolean(b) => h.write_val(&(4u8, *b)),
            rolldown_common::ConstantValue::Undefined => h.write_val(&5u8),
            rolldown_common::ConstantValue::Null => h.write_val(&6u8),
          }
        }
      };

    let symbol_db = &self.link_output.symbol_db;
    let sym_key = |s: rolldown_common::SymbolRef| {
      let c = symbol_db.canonical_ref_for(s);
      let s_idx = if symbol_db.is_facade_symbol(s) { u32::MAX } else { s.symbol.index() as u32 };
      let c_idx = if symbol_db.is_facade_symbol(c) { u32::MAX } else { c.symbol.index() as u32 };
      (
        s.owner.raw(),
        s_idx,
        c.owner.raw(),
        c_idx,
        chunk.canonical_names.get(&c).map(|n| n.as_str()),
        chunk.canonical_names.get(&s).map(|n| n.as_str()),
      )
    };

    let mut h = DualHasher::new();
    h.write_val(&(
      scan_gen,
      module_idx.raw(),
      module.ecma_view.dummy_record_set.len(),
      chunk.name.as_deref(),
      runtime_digest,
      meta.is_included,
      meta.wrap_kind() as u8,
      meta.wrapper_ref.map(&sym_key),
    ));
    if let Some(f) = chunk.preliminary_filename.as_ref() {
      rolldown_utils::hash_placeholder::visit_with_placeholders_defaulted(
        f.as_str(),
        &rolldown_utils::hash_placeholder::HASH_PLACEHOLDER_LEFT_FINDER,
        |_| true,
        |bytes| h.write_val(bytes),
      );
    }
    h.write_val(&(
      meta.wrapper_stmt_info.map(|s| s.raw()),
      meta.is_tla_or_contains_tla_dependency,
      meta.has_dynamic_exports,
      meta.module_namespace_included_reason.bits(),
      meta.namespace_included,
      meta.depended_runtime_helper.bits(),
      meta.native_default_expression,
      self.link_output.stmt_infos[module_idx].len(),
    ));
    match &chunk.kind {
      rolldown_common::ChunkKind::EntryPoint { meta: cmeta, module: emod, .. } => {
        h.write_val(&(1u8, cmeta.bits(), emod.raw()));
      }
      rolldown_common::ChunkKind::Common => {
        h.write_val(&0u8);
      }
    }
    for stmt_idx in 0..self.link_output.stmt_infos[module_idx].len() {
      let sidx = rolldown_common::StmtInfoIdx::from_raw_unchecked(stmt_idx as u32);
      h.write_val(&meta.stmt_info_included.has_bit(sidx));
    }
    let mut temp_acc = (0u128, 0u128);
    for (binding, flag) in &meta.temporal_symbols {
      let mut eh = DualHasher::new();
      eh.write_val(&(
        sym_key(*binding),
        sym_key(*flag),
        used_symbol_refs.contains(flag),
        meta.temporal_constants.contains(binding),
      ));
      let [d0, d1] = eh.finish();
      temp_acc = (temp_acc.0.wrapping_add(d0), temp_acc.1.wrapping_add(d1));
    }
    h.write_unordered(7, meta.temporal_symbols.len(), temp_acc);
    h.write_val(&(
      order_state.esm_init_target(module_idx, meta).map(|t| {
        (sym_key(t.wrapper_ref), t.tla_tainted, t.origin as u8)
      }),
      order_state.order_wrapper_chunk(module_idx) == Some(chunk_idx),
      final_esm_init_metadata.init_is_noop(module_idx),
      self
        .link_output
        .safely_merge_cjs_ns_map
        .get(&module_idx)
        .map(|info| info.needs_interop),
    ));
    if let Some(targets_by_stmt) = final_esm_init_metadata.transitive_init_targets(module_idx) {
      let mut t_acc = (0u128, 0u128);
      for (stmt_idx, targets) in targets_by_stmt {
        let mut eh = DualHasher::new();
        eh.write_val(&(stmt_idx.raw(), targets.len()));
        for target in targets {
          match target {
            crate::esm_init_obligations::WrappedEsmInitTarget::Module(m_idx) => {
              eh.write_val(&(
                1u8,
                m_idx.raw(),
                order_state.esm_init_target(*m_idx, &self.link_output.metas[*m_idx]).map(|t| {
                  (sym_key(t.wrapper_ref), t.tla_tainted, t.origin as u8)
                }),
                final_esm_init_metadata.init_is_noop(*m_idx),
              ));
            }
            crate::esm_init_obligations::WrappedEsmInitTarget::CjsCarrier(key) => {
              eh.write_val(&(
                2u8,
                key.importer.raw(),
                key.record.raw(),
                order_state.order_cjs_carrier(*key).map(|c| sym_key(c.wrapper_ref)),
              ));
            }
          }
        }
        let [d0, d1] = eh.finish();
        t_acc = (t_acc.0.wrapping_add(d0), t_acc.1.wrapping_add(d1));
      }
      h.write_unordered(15, targets_by_stmt.len(), t_acc);
    } else {
      h.write_val(&0u8);
    }

    h.write_val(&meta.referenced_symbols_by_entry_point_chunk.len());
    for (sym_ref, came_from_cjs) in &meta.referenced_symbols_by_entry_point_chunk {
      h.write_val(&(sym_key(*sym_ref), *came_from_cjs));
    }

    let mut ns_cjs_acc = (0u128, 0u128);
    for (k, v) in &meta.import_record_ns_to_cjs_module {
      let mut eh = DualHasher::new();
      eh.write_val(&(sym_key(*k), v.raw()));
      let [d0, d1] = eh.finish();
      ns_cjs_acc = (ns_cjs_acc.0.wrapping_add(d0), ns_cjs_acc.1.wrapping_add(d1));
    }
    h.write_unordered(11, meta.import_record_ns_to_cjs_module.len(), ns_cjs_acc);

    let mut mem_acc = (0u128, 0u128);
    for (node_id, res) in &meta.resolved_member_expr_refs {
      let mut eh = DualHasher::new();
      let resolved_info = res.resolved.map(|r| {
        let r_owner_exports = self.link_output.module_table[r.owner]
          .as_normal()
          .map(|m| m.exports_kind as u8);
        (
          sym_key(r),
          r.is_not_reassigned(symbol_db),
          self.link_output.metas[r.owner].wrap_kind() as u8,
          r_owner_exports,
          self
            .link_output
            .safely_merge_cjs_ns_map
            .get(&r.owner)
            .map(|i| i.needs_interop),
        )
      });
      eh.write_val(&(
        node_id.index(),
        resolved_info,
        res.target_commonjs_exported_symbol.map(|(s, b)| (sym_key(s), b)),
        res.prop_and_related_span_list.len(),
      ));
      if let Some((cjs_sym, _)) = res.target_commonjs_exported_symbol {
        hash_const_export_meta(
          &mut eh,
          self.link_output.global_constant_symbol_map.get(&cjs_sym),
        );
      }
      for prop in &res.prop_and_related_span_list {
        eh.write_val(&(prop.name.as_str(), prop.span.start, prop.span.end));
      }
      let [d0, d1] = eh.finish();
      mem_acc = (mem_acc.0.wrapping_add(d0), mem_acc.1.wrapping_add(d1));
    }
    h.write_unordered(12, meta.resolved_member_expr_refs.len(), mem_acc);

    let mut ns_in_acc = (0u128, 0u128);
    for (node_id, folded) in &meta.folded_namespace_in_checks {
      let mut eh = DualHasher::new();
      eh.write_val(&(node_id.index(), *folded));
      let [d0, d1] = eh.finish();
      ns_in_acc = (ns_in_acc.0.wrapping_add(d0), ns_in_acc.1.wrapping_add(d1));
    }
    h.write_unordered(16, meta.folded_namespace_in_checks.len(), ns_in_acc);

    let is_chunk_entry_module = matches!(
      chunk.kind,
      rolldown_common::ChunkKind::EntryPoint { module: em, .. } if em == module_idx
    );
    if meta.namespace_included || is_chunk_entry_module {
      let mut rexp_acc = (0u128, 0u128);
      for (name, export) in &meta.resolved_exports {
        let mut eh = DualHasher::new();
        let non_ambig = meta.sorted_and_non_ambiguous_resolved_exports.get(name).copied();
        let cref = symbol_db.canonical_ref_for(export.symbol_ref);
        eh.write_val(&(
          name.as_str(),
          export.came_from_commonjs,
          non_ambig,
          sym_key(export.symbol_ref),
          used_symbol_refs.contains(&export.symbol_ref),
          used_symbol_refs.contains(&cref),
          self.link_output.retained_export_symbols.contains(&export.symbol_ref),
          self.link_output.retained_export_symbols.contains(&cref),
        ));
        let [d0, d1] = eh.finish();
        rexp_acc = (rexp_acc.0.wrapping_add(d0), rexp_acc.1.wrapping_add(d1));
      }
      h.write_unordered(9, meta.resolved_exports.len(), rexp_acc);
    } else {
      h.write_val(&0u8);
    }

    let hash_sym_full = |h: &mut DualHasher, sym_ref: rolldown_common::SymbolRef| {
      let canonical_ref = symbol_db.canonical_ref_for(sym_ref);
      let sym = symbol_db.get(canonical_ref);
      let ns_alias = sym.namespace_alias.as_ref().map(|na| {
        let ns_canon = symbol_db.canonical_ref_for(na.namespace_ref);
        (
          na.property_name.as_str(),
          sym_key(ns_canon),
          cache.module_scan_generation(ns_canon.owner),
          self
            .link_output
            .safely_merge_cjs_ns_map
            .get(&ns_canon.owner)
            .map(|i| i.needs_interop),
        )
      });
      let temporal_guard = self.link_output.metas[canonical_ref.owner]
        .temporal_symbols
        .get(&canonical_ref)
        .map(|flag| (sym_key(*flag), used_symbol_refs.contains(flag)));
      h.write_val(&(
        sym_key(sym_ref),
        cache.module_scan_generation(canonical_ref.owner),
        used_symbol_refs.contains(&sym_ref),
        used_symbol_refs.contains(&canonical_ref),
        self.link_output.retained_export_symbols.contains(&sym_ref),
        self.link_output.retained_export_symbols.contains(&canonical_ref),
        symbol_db.is_facade_symbol(sym_ref),
        symbol_db.is_facade_symbol(canonical_ref),
        sym_ref.is_not_reassigned(symbol_db),
        canonical_ref.is_not_reassigned(symbol_db),
        sym.chunk_idx.map(|c| c.raw()),
        ns_alias,
      ));
      h.write_val(&temporal_guard);
      hash_const_export_meta(h, self.link_output.global_constant_symbol_map.get(&sym_ref));
      hash_const_export_meta(
        h,
        self.link_output.global_constant_symbol_map.get(&canonical_ref),
      );
    };
    let local_sym_count =
      symbol_db[module_idx].as_ref().map_or(0, |db| db.classic_data.len());
    let mut seen_local_syms = vec![false; local_sym_count];
    let mut seen_other_syms = FxHashSet::default();
    let mut hash_sym = |h: &mut DualHasher, sym_ref: rolldown_common::SymbolRef| {
      if sym_ref.owner == module_idx && !symbol_db.is_facade_symbol(sym_ref) {
        let idx = sym_ref.symbol.index();
        if idx < seen_local_syms.len() {
          if std::mem::replace(&mut seen_local_syms[idx], true) {
            return;
          }
          hash_sym_full(h, sym_ref);
          return;
        }
      }
      let key = (
        sym_ref.owner.raw(),
        if symbol_db.is_facade_symbol(sym_ref) {
          u32::MAX
        } else {
          sym_ref.symbol.index() as u32
        },
      );
      if seen_other_syms.insert(key) {
        hash_sym_full(h, sym_ref);
      }
    };

    hash_sym(&mut h, module.namespace_object_ref);
    hash_sym(&mut h, module.default_export_ref);
    if let Some(hmr_hot_ref) = module.ecma_view.hmr_hot_ref {
      hash_sym(&mut h, hmr_hot_ref);
    }
    let mut nimp_acc = (0u128, 0u128);
    for (&sym_ref, named_import) in &module.named_imports {
      let mut eh = DualHasher::new();
      hash_sym_full(&mut eh, sym_ref);
      eh.write_val(&named_import.record_idx.raw());
      let [d0, d1] = eh.finish();
      nimp_acc = (nimp_acc.0.wrapping_add(d0), nimp_acc.1.wrapping_add(d1));
    }
    h.write_unordered(13, module.named_imports.len(), nimp_acc);

    for stmt_info in &self.link_output.stmt_infos[module_idx].infos {
      for sym_ref in &stmt_info.declared_symbols {
        hash_sym(&mut h, sym_ref.inner());
      }
      for sym_or_mem in &stmt_info.referenced_symbols {
        match sym_or_mem {
          rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
            hash_sym(&mut h, *sym_ref);
          }
          rolldown_common::SymbolOrMemberExprRef::MemberExpr(mem_ref) => {
            hash_sym(&mut h, mem_ref.object_ref);
            if let Some(res) = mem_ref.resolution(&meta.resolved_member_expr_refs)
              && let Some(resolved) = res.resolved
            {
              hash_sym(&mut h, resolved);
            }
          }
        }
      }
    }

    h.write_val(&module.import_records.len());
    for (rec_idx, rec) in module.import_records.iter_enumerated() {
      hash_sym(&mut h, rec.namespace_ref);
      let merged_cjs_ns = chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx]
        .get(&rec.namespace_ref)
        .copied()
        .map(&sym_key);
      let carrier_key = super::order_wrap_state::OrderCjsCarrierKey {
        importer: module_idx,
        record: rec_idx,
      };
      let carrier_wrapper = order_state.order_cjs_carrier(carrier_key).map(|c| sym_key(c.wrapper_ref));
      h.write_val(&(
        rec.kind as u8,
        rec.meta.bits(),
        rec.resolved_module.map(|m| m.raw()),
        merged_cjs_ns,
        carrier_wrapper,
      ));
      if let Some(importee_idx) = rec.resolved_module {
        let importee_mod = &self.link_output.module_table[importee_idx];
        h.write_val(&importee_mod.stable_id().as_str());
        if let Some(ext) = importee_mod.as_external() {
          h.write_val(&(ext.get_import_path(chunk, None), ext.get_file_name(None)));
        } else if let Some(importee_normal) = importee_mod.as_normal() {
          let importee_meta = &self.link_output.metas[importee_idx];
          let importee_chunk_file = chunk_graph.module_to_chunk[importee_idx]
            .and_then(|cidx| chunk_graph.chunk_table.get(cidx))
            .and_then(|c| c.preliminary_filename.as_ref().map(|f| f.as_str()));
          let importee_entry_chunk_file = chunk_graph
            .entry_module_to_entry_chunk
            .get(&importee_idx)
            .and_then(|cidx| chunk_graph.chunk_table.get(*cidx))
            .and_then(|c| c.preliminary_filename.as_ref().map(|f| f.as_str()));
          let importee_init = order_state.esm_init_target(importee_idx, importee_meta).map(|t| {
            (sym_key(t.wrapper_ref), t.tla_tainted, t.origin as u8)
          });
          h.write_val(&(
            importee_normal.exports_kind as u8,
            importee_meta.wrap_kind() as u8,
            importee_meta.wrapper_ref.map(&sym_key),
            importee_meta
              .wrapper_stmt_info
              .is_some_and(|s| importee_meta.stmt_info_included.has_bit(s)),
            importee_init,
            final_esm_init_metadata.init_is_noop(importee_idx),
            chunk_graph.module_to_chunk[importee_idx] == Some(chunk_idx),
          ));
          h.write_val(&(
            importee_meta.is_included,
            importee_meta.is_tla_or_contains_tla_dependency,
            importee_meta.has_dynamic_exports,
            importee_chunk_file,
            importee_entry_chunk_file,
            self
              .link_output
              .safely_merge_cjs_ns_map
              .get(&importee_idx)
              .map(|info| info.needs_interop),
          ));
          if rec.meta.contains(rolldown_common::ImportRecordMeta::IsExportStar)
            || cache.export_star_modules.contains(&importee_idx)
          {
            h.write_val(&cache.module_scan_generation(importee_idx));
          }
          if let Some((fexp_count, fexp_acc)) = importee_reexport_wrap_digest[importee_idx] {
            h.write_unordered(14, fexp_count, fexp_acc);
          }
          h.write_val(&(
            order_state.is_consumer_local_reexport_route(importee_idx),
            order_state.reexport_init_is_transparent(importee_idx),
          ));
        }
      }
    }

    let mut ov_acc = (0u128, 0u128);
    let mut ov_count = 0usize;
    if order_state.module_has_import_overlays(module_idx) {
      for (key, overlay) in order_state.import_overlays_for_importer(module_idx) {
        ov_count += 1;
        let mut eh = DualHasher::new();
        eh.write_val(&(
          key.statement.raw(),
          key.record.raw(),
          overlay.runtime_helpers.bits(),
          overlay.requires_importer_namespace,
          overlay.requires_importee_namespace,
          overlay.reexports_dynamic_exports,
          overlay.referenced_symbols.len(),
          overlay.retained_reexport_path.len(),
        ));
        for &sym in &overlay.referenced_symbols {
          eh.write_val(&sym_key(sym));
        }
        for &(m_idx, r_idx) in &overlay.retained_reexport_path {
          eh.write_val(&(m_idx.raw(), r_idx.raw()));
        }
        let [d0, d1] = eh.finish();
        ov_acc = (ov_acc.0.wrapping_add(d0), ov_acc.1.wrapping_add(d1));
      }
    }
    h.write_unordered(16, ov_count, ov_acc);

    Some(h.finish())
  }
}
