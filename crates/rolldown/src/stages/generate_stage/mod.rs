use std::{collections::hash_map::Entry, sync::Arc};

use arcstr::ArcStr;
use futures::future::try_join_all;
use oxc::{
  ast_visit::VisitMut,
  semantic::{ScopeId, SymbolId},
};
use oxc_index::IndexVec;
use render_chunk_to_assets::set_emitted_chunk_preliminary_filenames;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_std_utils::OptionExt;
use rustc_hash::{FxHashMap, FxHashSet};

use rolldown_common::{
  ChunkIdx, ChunkKind, CssAssetNameReplacer, ImportMetaRolldownAssetReplacer, Module, ModuleIdx,
  PreliminaryFilename, RollupPreRenderedAsset,
};
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::{PathBufExt, PathExt};
use rolldown_utils::{
  concat_string,
  hash_placeholder::HashPlaceholderGenerator,
  index_vec_ext::IndexVecRefExt,
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};
use sugar_path::SugarPath;

use crate::{
  BundleOutput, SharedOptions,
  chunk_graph::ChunkGraph,
  module_finalizers::{
    isolating::{IsolatingModuleFinalizer, IsolatingModuleFinalizerContext},
    scope_hoisting::ScopeHoistingFinalizerContext,
  },
  stages::link_stage::LinkStageOutput,
  utils::{
    chunk::{
      deconflict_chunk_symbols::deconflict_chunk_symbols, generate_pre_rendered_chunk,
      validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
    },
    finalize_normal_module,
  },
};

mod advanced_chunks;
mod code_splitting;
mod compute_cross_chunk_links;
mod minify_assets;
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
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.plugin_driver.render_start(self.options).await?;

    let mut chunk_graph = self.generate_chunks().await;
    if chunk_graph.chunk_table.len() > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    self.compute_cross_chunk_links(&mut chunk_graph);

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

    let ast_table_iter = self.link_output.ast_table.par_iter_mut();
    ast_table_iter
      .filter(|(_ast, owner)| {
        self.link_output.module_table.modules[*owner]
          .as_normal()
          .is_some_and(|m| m.meta.is_included())
      })
      .for_each(|(ast, owner)| {
        let Module::Normal(module) = &self.link_output.module_table.modules[*owner] else {
          return;
        };
        let ast_scope = &self.link_output.symbol_db[module.idx].as_ref().unwrap().ast_scopes;
        let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
        let chunk = &chunk_graph.chunk_table[chunk_id];
        let linking_info = &self.link_output.metas[module.idx];
        if self.options.format.requires_scope_hoisting() {
          finalize_normal_module(
            ScopeHoistingFinalizerContext {
              canonical_names: &chunk.canonical_names,
              id: module.idx,
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
            },
            ast,
            ast_scope,
          );
        } else {
          ast.program.with_mut(|fields| {
            let (oxc_program, alloc) = (fields.program, fields.allocator);
            let mut finalizer = IsolatingModuleFinalizer {
              alloc,
              scope: ast_scope,
              ctx: &IsolatingModuleFinalizerContext {
                module,
                modules: &self.link_output.module_table.modules,
                symbol_db: &self.link_output.symbol_db,
              },
              snippet: AstSnippet::new(alloc),
              generated_imports_set: FxHashSet::default(),
              generated_imports: oxc::allocator::Vec::new_in(alloc),
              generated_exports: oxc::allocator::Vec::new_in(alloc),
            };
            finalizer.visit_program(oxc_program);
          });
        }
      });

    self.render_chunk_to_assets(&mut chunk_graph).await
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
          return anyhow::Ok(name.clone());
        }
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, is_user_defined, .. } => {
            let module = &modules[entry_module_id];
            let generated = if is_user_defined {
              // try extract meaningful input name from path
              if let Some(file_stem) = module.id().as_path().file_stem().and_then(|f| f.to_str()) {
                sanitize_filename.call(file_stem).await?
              } else {
                arcstr::literal!("input")
              }
            } else {
              sanitize_filename.call(&module.id().as_path().representative_file_name()).await?
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
              Ok(sanitize_filename.call(&module.id().as_path().representative_file_name()).await?)
            } else {
              Ok(arcstr::literal!("chunk"))
            }
          }
        }
      }
    });

    let mut index_pre_generated_names: IndexVec<ChunkIdx, ArcStr> =
      try_join_all(index_pre_generated_names_futures).await?.into();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();

    let create_make_unique_name = |mut used_name_counts: FxHashMap<ArcStr, u32>| {
      move |name: &ArcStr| {
        let mut candidate = name.clone();
        loop {
          match used_name_counts.entry(candidate.clone()) {
            Entry::Occupied(mut occ) => {
              // This name is already used
              let next_count = *occ.get();
              occ.insert(next_count + 1);
              candidate =
                ArcStr::from(concat_string!(name, itoa::Buffer::new().format(next_count)).as_str());
            }
            Entry::Vacant(vac) => {
              // This is the first time we see this name
              let name = vac.key().clone();
              vac.insert(2);
              break name;
            }
          }
        }
      }
    };
    let mut make_unique_name_for_ecma_chunk = create_make_unique_name(FxHashMap::default());
    let mut make_unique_name_for_css_chunk = create_make_unique_name(FxHashMap::default());

    for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &mut chunk_graph.chunk_table[*chunk_id];
      if chunk.preliminary_filename.is_some() {
        // Already generated
        continue;
      }

      let pre_generated_chunk_name = &mut index_pre_generated_names[*chunk_id];
      // Notice we didn't used deconflict name here, chunk names are allowed to be duplicated.
      chunk.name = Some(pre_generated_chunk_name.clone());
      index_chunk_id_to_name.insert(*chunk_id, pre_generated_chunk_name.clone());
      let pre_rendered_chunk = generate_pre_rendered_chunk(chunk, self.link_output);

      let preliminary_filename = chunk
        .generate_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          pre_generated_chunk_name,
          &mut hash_placeholder_generator,
          &mut make_unique_name_for_ecma_chunk,
        )
        .await?;

      let css_preliminary_filename = chunk
        .generate_css_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          pre_generated_chunk_name,
          &mut hash_placeholder_generator,
          &mut make_unique_name_for_css_chunk,
        )
        .await?;

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

          let filename = asset_filename_template.render(Some(&name), extension, hash_replacer);
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
        let Module::Normal(module) = &mut self.link_output.module_table.modules[*module_idx] else {
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
        let module = &mut self.link_output.module_table.modules[*module_idx];
        if let Some(css_view) =
          module.as_normal_mut().and_then(|normal_module| normal_module.css_view.as_mut())
        {
          for (idx, record) in
            css_view.import_records.iter_enumerated().filter(|(_idx, rec)| !rec.is_dummy())
          {
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
}
