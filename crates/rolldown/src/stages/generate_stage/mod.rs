use std::collections::hash_map::Entry;

use arcstr::ArcStr;
use oxc::ast::VisitMut;
use oxc_index::IndexVec;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_std_utils::OptionExt;
use rustc_hash::{FxHashMap, FxHashSet};

use rolldown_common::{
  ChunkIdx, ChunkKind, CssAssetNameReplacer, FileNameRenderOptions,
  ImportMetaRolldownAssetReplacer, Module, PreliminaryFilename,
};
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::{PathBufExt, PathExt};
use rolldown_utils::{
  concat_string,
  extract_hash_pattern::extract_hash_pattern,
  hash_placeholder::HashPlaceholderGenerator,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
  sanitize_file_name::sanitize_file_name,
};
use sugar_path::SugarPath;

use crate::{
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
    extract_meaningful_input_name_from_path::try_extract_meaningful_input_name_from_path,
    finalize_normal_module,
  },
  BundleOutput, SharedOptions,
};

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

    let mut chunk_graph = self.generate_chunks().await?;
    if chunk_graph.chunk_table.len() > 1 {
      validate_options_for_multi_chunk_output(self.options)?;
    }

    self.compute_cross_chunk_links(&mut chunk_graph);

    let index_chunk_id_to_name =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;
    self.patch_asset_modules(&chunk_graph);

    chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
      deconflict_chunk_symbols(
        chunk,
        self.link_output,
        self.options.format,
        &index_chunk_id_to_name,
      );
    });

    let ast_table_iter = self.link_output.ast_table.par_iter_mut();
    ast_table_iter
      .filter(|(_ast, owner)| {
        self.link_output.module_table.modules[*owner]
          .as_normal()
          .map_or(false, |m| m.meta.is_included())
      })
      .for_each(|(ast, owner)| {
        let Module::Normal(module) = &self.link_output.module_table.modules[*owner] else {
          return;
        };
        let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
        let chunk = &chunk_graph.chunk_table[chunk_id];
        let linking_info = &self.link_output.metas[module.idx];
        if self.options.format.requires_scope_hoisting() {
          finalize_normal_module(
            module,
            ScopeHoistingFinalizerContext {
              canonical_names: &chunk.canonical_names,
              id: module.idx,
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
            },
            ast,
          );
        } else {
          ast.program.with_mut(|fields| {
            let (oxc_program, alloc) = (fields.program, fields.allocator);
            let mut finalizer = IsolatingModuleFinalizer {
              alloc,
              scope: &module.scope,
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
  ) -> anyhow::Result<FxHashMap<ChunkIdx, ArcStr>> {
    let modules = &self.link_output.module_table.modules;

    let mut index_chunk_id_to_name = FxHashMap::default();
    let mut index_pre_generated_names: IndexVec<ChunkIdx, ArcStr> = chunk_graph
      .chunk_table
      .par_iter()
      .map(|chunk| {
        if let Some(name) = &chunk.name {
          return name.clone();
        }
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, is_user_defined, .. } => {
            let module = &modules[entry_module_id];
            let generated = if is_user_defined {
              try_extract_meaningful_input_name_from_path(module.id())
                .map(ArcStr::from)
                .unwrap_or(arcstr::literal!("input"))
            } else {
              ArcStr::from(sanitize_file_name(module.id().as_path().representative_file_name()))
            };
            generated
          }
          ChunkKind::Common => {
            // - rollup use the first entered/last executed module as the `[name]` of common chunks.
            // - esbuild always use 'chunk' as the `[name]`. However we try to make the name more meaningful here.
            let first_executed_non_runtime_module =
              chunk.modules.iter().rev().find(|each| **each != self.link_output.runtime.id());
            first_executed_non_runtime_module.map_or_else(
              || arcstr::literal!("chunk"),
              |module_id| {
                let module = &modules[*module_id];
                ArcStr::from(sanitize_file_name(module.id().as_path().representative_file_name()))
              },
            )
          }
        }
      })
      .collect::<Vec<_>>()
      .into();

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
          };
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
      let pre_rendered_chunk = generate_pre_rendered_chunk(chunk, self.link_output, self.options);

      let asset_filename_template = &self.options.asset_filenames;
      let extracted_asset_hash_pattern = extract_hash_pattern(asset_filename_template.template());

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

      chunk.modules.iter().copied().filter_map(|idx| modules[idx].as_normal()).for_each(|module| {
        if module.asset_view.is_some() {
          let hash_placeholder = extracted_asset_hash_pattern
            .as_ref()
            .map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));
          let name = module.id.as_path().file_stem().and_then(|s| s.to_str()).unpack();
          let preliminary = PreliminaryFilename::new(
            asset_filename_template.render(&FileNameRenderOptions {
              name: Some(name),
              hash: hash_placeholder.as_deref(),
              ext: module.id.as_path().extension().and_then(|s| s.to_str()),
            }),
            hash_placeholder,
          );

          chunk.asset_absolute_preliminary_filenames.insert(
            module.idx,
            preliminary
              .absolutize_with(self.options.cwd.join(&self.options.dir))
              .expect_into_string(),
          );
          chunk.asset_preliminary_filenames.insert(module.idx, preliminary);
        }
      });

      chunk.pre_rendered_chunk = Some(pre_rendered_chunk);

      chunk.absolute_preliminary_filename = Some(
        preliminary_filename
          .absolutize_with(self.options.cwd.join(&self.options.dir))
          .expect_into_string(),
      );
      chunk.css_absolute_preliminary_filename = Some(
        css_preliminary_filename
          .absolutize_with(self.options.cwd.join(&self.options.dir))
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
        module.ecma_view.mutations.push(Box::new(ImportMetaRolldownAssetReplacer {
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
          for (idx, record) in css_view.import_records.iter_enumerated() {
            if let Some(asset_filename) = module_idx_to_filenames.get(&record.resolved_module) {
              let span = css_view.record_idx_to_span[idx];
              css_view
                .mutations
                .push(Box::new(CssAssetNameReplacer { span, asset_name: asset_filename.clone() }));
            }
          }
        }
      });
    });
  }
}
