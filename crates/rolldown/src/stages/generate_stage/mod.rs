use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::try_join_all;
use oxc::semantic::{ScopeId, SymbolId};
use oxc_index::IndexVec;
use render_chunk_to_assets::set_emitted_chunk_preliminary_filenames;
use rolldown_debug::{action, trace_action, trace_action_enabled};
use rolldown_error::BuildResult;
use rolldown_std_utils::OptionExt;
use rustc_hash::FxHashMap;

use rolldown_common::{
  ChunkIdx, ChunkKind, ConcatenateWrappedModuleKind, CssAssetNameReplacer,
  ImportMetaRolldownAssetReplacer, ImportRecordIdx, Module, ModuleIdx, PreliminaryFilename,
  PrependRenderedImport, RenderedConcatenatedModuleParts, RollupPreRenderedAsset,
};
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::{PathBufExt, PathExt, representative_file_name_for_preserve_modules};
use rolldown_utils::{
  dashmap::FxDashMap,
  hash_placeholder::HashPlaceholderGenerator,
  index_vec_ext::{IndexVecExt, IndexVecRefExt},
  indexmap::FxIndexMap,
  make_unique_name::make_unique_name,
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};
use sugar_path::SugarPath;

use crate::{
  BundleOutput, SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::ScopeHoistingFinalizerContext,
  stages::link_stage::LinkStageOutput,
  utils::chunk::{
    deconflict_chunk_symbols::deconflict_chunk_symbols, generate_pre_rendered_chunk,
    validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
  },
};

mod advanced_chunks;
mod chunk_ext;
mod code_splitting;
mod compute_cross_chunk_links;
mod minify_assets;
mod on_demand_wrapping;
mod render_chunk_to_assets;

pub struct GenerateStage<'a> {
  link_output: &'a mut LinkStageOutput,
  options: &'a SharedOptions,
  plugin_driver: &'a SharedPluginDriver,
}

impl<'a> GenerateStage<'a> {
  pub fn new(
    link_output: &'a mut LinkStageOutput,
    options: &'a SharedOptions,
    plugin_driver: &'a SharedPluginDriver,
  ) -> Self {
    Self { link_output, options, plugin_driver }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  #[allow(clippy::too_many_lines)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.plugin_driver.render_start(self.options).await?;

    let mut chunk_graph = self.generate_chunks().await?;

    if chunk_graph.chunk_table.len() > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    self.compute_cross_chunk_links(&mut chunk_graph);

    self.ensure_lazy_module_initialization_order(&mut chunk_graph);

    self.on_demand_wrapping(&mut chunk_graph);

    self.trace_action_chunks_infos(&chunk_graph);

    let index_chunk_id_to_name =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;
    self.patch_asset_modules(&chunk_graph);
    set_emitted_chunk_preliminary_filenames(&self.plugin_driver.file_emitter, &chunk_graph);

    let module_scope_symbol_id_map = self
      .link_output
      .symbol_db
      .inner()
      .par_iter_enumerated()
      .filter_map(|(idx, db)| {
        let Some(db) = db else {
          return None;
        };
        let root_scope_id = db.ast_scopes.scoping().root_scope_id();
        let mut vec: IndexVec<ScopeId, Vec<(SymbolId, &str)>> =
          IndexVec::from_vec(vec![vec![]; db.ast_scopes.scoping().scopes_len()]);
        for symbol_id in db.scoping().symbol_ids() {
          let scope_id = db.scoping().symbol_scope_id(symbol_id);
          if scope_id == root_scope_id {
            continue;
          }
          vec[scope_id].push((symbol_id, db.scoping().symbol_name(symbol_id)));
        }
        Some((idx, vec))
      })
      .collect::<FxHashMap<ModuleIdx, IndexVec<ScopeId, Vec<(SymbolId, &str)>>>>();

    chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
      deconflict_chunk_symbols(
        chunk,
        self.link_output,
        self.options.format,
        &index_chunk_id_to_name,
        &module_scope_symbol_id_map,
      );
    });

    let ast_table_iter = self.link_output.ast_table.par_iter_mut_enumerated();
    let transfer_parts_rendered_maps = ast_table_iter
      .filter(|(idx, _ast)| {
        self.link_output.module_table[*idx].as_normal().is_some_and(|m| m.meta.is_included())
      })
      .filter_map(|(idx, ast)| {
        let Some(ast) = ast else {
          return None;
        };
        let module = self.link_output.module_table[idx].as_normal().unwrap();
        let ast_scope = &self.link_output.symbol_db[idx].as_ref().unwrap().ast_scopes;
        let chunk_id = chunk_graph.module_to_chunk[idx].unwrap();
        let chunk = &chunk_graph.chunk_table[chunk_id];
        let linking_info = &self.link_output.metas[module.idx];
        let ctx = ScopeHoistingFinalizerContext {
          canonical_names: &chunk.canonical_names,
          id: idx,
          chunk_id,
          symbol_db: &self.link_output.symbol_db,
          linking_info,
          module,
          modules: &self.link_output.module_table.modules,
          linking_infos: &self.link_output.metas,
          runtime: &self.link_output.runtime,
          chunk_graph: &chunk_graph,
          options: self.options,
          cur_stmt_index: 0,
          keep_name_statement_to_insert: Vec::new(),
          file_emitter: &self.plugin_driver.file_emitter,
          constant_value_map: &self.link_output.constant_symbol_map,
          needs_hosted_top_level_binding: false,
          module_namespace_included: self
            .link_output
            .used_symbol_refs
            .contains(&module.namespace_object_ref),
          transferred_import_record: chunk
            .remove_map
            .get(&module.idx)
            .cloned()
            .map(|idxs| {
              idxs.into_iter().map(|idx| (idx, String::new())).collect::<FxIndexMap<_, _>>()
            })
            .unwrap_or_default(),
          rendered_concatenated_wrapped_module_parts: RenderedConcatenatedModuleParts::default(),
        };
        let ctx = ctx.finalize_normal_module(ast, ast_scope);
        (!ctx.transferred_import_record.is_empty()
          || !matches!(
            ctx.linking_info.concatenated_wrapped_module_kind,
            ConcatenateWrappedModuleKind::None
          ))
        .then_some((
          idx,
          ctx.transferred_import_record,
          ctx.rendered_concatenated_wrapped_module_parts,
        ))
      })
      .collect::<Vec<_>>();

    self.apply_transfer_parts_mutation(&mut chunk_graph, transfer_parts_rendered_maps);
    self.render_chunk_to_assets(&chunk_graph).await
  }

  fn apply_transfer_parts_mutation(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    transfer_parts_rendered_maps: Vec<(
      ModuleIdx,
      FxIndexMap<ImportRecordIdx, String>,
      RenderedConcatenatedModuleParts,
    )>,
  ) {
    let mut normalized_transfer_parts_rendered_maps = FxHashMap::default();
    for (idx, transferred_import_record, rendered_concatenated_module_parts) in
      transfer_parts_rendered_maps
    {
      for (rec_idx, rendered_string) in transferred_import_record {
        normalized_transfer_parts_rendered_maps.insert((idx, rec_idx), rendered_string);
      }
      let chunk_idx = chunk_graph.module_to_chunk[idx].expect("should have chunk idx");
      let chunk = &mut chunk_graph.chunk_table[chunk_idx];
      chunk
        .module_idx_to_render_concatenated_module
        .insert(idx, rendered_concatenated_module_parts);
    }

    if normalized_transfer_parts_rendered_maps.is_empty() {
      return;
    }
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

  /// Notices:
  /// - Should generate filenames that are stable cross builds and os.
  #[tracing::instrument(level = "debug", skip_all)]
  #[allow(clippy::too_many_lines)]
  async fn generate_chunk_name_and_preliminary_filenames(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<FxHashMap<ChunkIdx, ArcStr>> {
    let modules = &self.link_output.module_table.modules;

    let mut index_chunk_id_to_name = FxHashMap::default();

    let index_pre_generated_names_futures = chunk_graph.chunk_table.iter().map(|chunk| {
      let sanitize_filename = self.options.sanitize_filename.clone();
      async move {
        if let Some(name) = &chunk.name {
          let name = sanitize_filename.call(name).await?;
          return anyhow::Ok((name.clone(), name));
        }
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, meta, .. } => {
            let module = &modules[entry_module_id];
            let generated = if self.options.preserve_modules {
              let module_id = module.id();
              let (chunk_name, absolute_chunk_file_name) =
                representative_file_name_for_preserve_modules(module_id.as_path());

              let sanitized_absolute_filename =
                sanitize_filename.call(absolute_chunk_file_name.as_str()).await?;

              let sanitized_chunk_name = sanitize_filename.call(&chunk_name).await?;
              (sanitized_chunk_name, sanitized_absolute_filename)
            } else if meta.contains(rolldown_common::ChunkMeta::UserDefinedEntry) {
              // try extract meaningful input name from path
              if let Some(file_stem) = module.id().as_path().file_stem().and_then(|f| f.to_str()) {
                let name = sanitize_filename.call(file_stem).await?;
                (name.clone(), name)
              } else {
                let name = arcstr::literal!("input");
                (name.clone(), name)
              }
            } else {
              let chunk_name =
                sanitize_filename.call(&module.id().as_path().representative_file_name()).await?;

              (chunk_name.clone(), chunk_name)
            };
            Ok(generated)
          }
          ChunkKind::Common => {
            // - rollup use the first entered/last executed module as the `[name]` of common chunks.
            // - esbuild always use 'chunk' as the `[name]`. However we try to make the name more meaningful here.
            if let Some(module_id) =
              chunk.modules.iter().rev().find(|each| **each != self.link_output.runtime.id())
            {
              let module = &modules[*module_id];
              let module_id = module.id();
              let name = module_id.as_path().representative_file_name();
              let sanitized_filename = sanitize_filename.call(&name).await?;
              Ok((sanitized_filename.clone(), sanitized_filename))
            } else {
              let name = arcstr::literal!("chunk");
              Ok((name.clone(), name))
            }
          }
        }
      }
    });

    // First one is chunk_name, the second one is chunk filename
    let mut index_pre_generated_names: IndexVec<ChunkIdx, (ArcStr, ArcStr)> =
      try_join_all(index_pre_generated_names_futures).await?.into();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();

    let used_name_counts = FxDashMap::default();

    for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &mut chunk_graph.chunk_table[*chunk_id];
      if chunk.preliminary_filename.is_some() {
        // Already generated
        continue;
      }

      let pre_generated_chunk_name = &mut index_pre_generated_names[*chunk_id];
      // Notice we didn't used deconflict name here, chunk names are allowed to be duplicated.
      index_chunk_id_to_name.insert(*chunk_id, pre_generated_chunk_name.0.clone());
      let pre_rendered_chunk =
        generate_pre_rendered_chunk(chunk, &pre_generated_chunk_name.0, self.link_output);

      let preliminary_filename = chunk
        .generate_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          &pre_generated_chunk_name.1,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;

      let css_preliminary_filename = chunk
        .generate_css_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          &pre_generated_chunk_name.1,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;

      // Defer chunk name assignment to make sure at this point only entry chunk have a name
      // if user provided one.
      chunk.name = Some(pre_generated_chunk_name.0.clone());

      for module in chunk.modules.iter().copied().filter_map(|idx| modules[idx].as_normal()) {
        if let Some(asset_view) = module.asset_view.as_ref() {
          let name = self
            .options
            .sanitize_filename
            .call(module.id.as_path().file_stem().and_then(|s| s.to_str()).unpack())
            .await?;
          let asset_filename_template = self
            .options
            .asset_filename_template(&RollupPreRenderedAsset {
              names: vec![name.clone()],
              original_file_names: vec![],
              source: asset_view.source.to_vec().into(),
            })
            .await?;

          let has_hash_pattern = asset_filename_template.has_hash_pattern();
          let extension = module.id.as_path().extension().and_then(|s| s.to_str());

          let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
          let hash_replacer = has_hash_pattern.then_some({
            |len: Option<usize>| {
              let hash = hash_placeholder_generator.generate(len);
              if let Some(hash_placeholder) = hash_placeholder.as_mut() {
                hash_placeholder.push(hash.clone());
              }
              hash
            }
          });

          let mut filename =
            asset_filename_template.render(Some(&name), extension, hash_replacer).into();
          filename = make_unique_name(&filename, &used_name_counts);
          let preliminary = PreliminaryFilename::new(filename, hash_placeholder);

          chunk.asset_absolute_preliminary_filenames.insert(
            module.idx,
            preliminary
              .absolutize_with(self.options.cwd.join(&self.options.out_dir))
              .expect_into_string(),
          );
          chunk.asset_preliminary_filenames.insert(module.idx, preliminary);
        }
      }

      chunk.pre_rendered_chunk = Some(pre_rendered_chunk);

      chunk.absolute_preliminary_filename = Some(
        preliminary_filename
          .absolutize_with(self.options.cwd.join(&self.options.out_dir))
          .expect_into_string(),
      );
      chunk.css_absolute_preliminary_filename = Some(
        css_preliminary_filename
          .absolutize_with(self.options.cwd.join(&self.options.out_dir))
          .expect_into_string(),
      );
      chunk.preliminary_filename = Some(preliminary_filename);
      chunk.css_preliminary_filename = Some(css_preliminary_filename);
    }
    Ok(index_chunk_id_to_name)
  }

  pub fn patch_asset_modules(&mut self, chunk_graph: &ChunkGraph) {
    chunk_graph.chunk_table.iter().for_each(|chunk| {
      let mut module_idx_to_filenames = FxHashMap::default();
      // replace asset name in ecma view
      chunk.asset_preliminary_filenames.iter().for_each(|(module_idx, preliminary)| {
        let Module::Normal(module) = &mut self.link_output.module_table[*module_idx] else {
          return;
        };
        let asset_filename: ArcStr = preliminary.as_str().into();
        module.ecma_view.mutations.push(Arc::new(ImportMetaRolldownAssetReplacer {
          asset_filename: asset_filename.clone(),
        }));
        module_idx_to_filenames.insert(module_idx, asset_filename);
      });
      // replace asset name in css view
      chunk.modules.iter().for_each(|module_idx| {
        let module = &mut self.link_output.module_table[*module_idx];
        if let Some(css_view) =
          module.as_normal_mut().and_then(|normal_module| normal_module.css_view.as_mut())
        {
          for (idx, record) in css_view.import_records.iter_enumerated() {
            if let Some(asset_filename) = module_idx_to_filenames.get(&record.resolved_module) {
              let span = css_view.record_idx_to_span[idx];
              css_view
                .mutations
                .push(Arc::new(CssAssetNameReplacer { span, asset_name: asset_filename.clone() }));
            }
          }
        }
      });
    });
  }

  fn trace_action_chunks_infos(&self, chunk_graph: &ChunkGraph) {
    if trace_action_enabled!() {
      let mut chunk_infos = Vec::new();
      for (idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        chunk_infos.push(action::Chunk {
          is_user_defined_entry: chunk.is_user_defined_entry(),
          is_async_entry: chunk.is_async_entry(),
          entry_module: chunk
            .entry_module_idx()
            .map(|idx| self.link_output.module_table[idx].id().to_string()),
          modules: chunk
            .modules
            .iter()
            .map(|idx| self.link_output.module_table[*idx].id().to_string())
            .collect(),
          reason: chunk.chunk_reason_type.as_static_str(),
          advanced_chunk_group_id: chunk.chunk_reason_type.group_index(),
          chunk_id: idx.raw(),
          name: chunk.name.as_ref().map(ArcStr::to_string),
          // TODO(hyf0): add dynamic importees
          imports: chunk
            .imports_from_other_chunks
            .iter()
            .map(|(importee_idx, _imports)| action::ChunkImport {
              chunk_id: importee_idx.raw(),
              kind: "import-statement",
            })
            .collect(),
        });
      }
      trace_action!(action::ChunkGraphReady { action: "ChunkGraphReady", chunks: chunk_infos });
    }
  }
}
