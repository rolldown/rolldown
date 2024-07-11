use anyhow::Result;
use indexmap::IndexSet;
use oxc::{ast::VisitMut, index::IndexVec};
use rolldown_ecmascript::AstSnippet;
use rustc_hash::FxHashSet;

use futures::future::try_join_all;
use rolldown_common::{
  ChunkIdx, ChunkKind, FileNameRenderOptions, Module, Output, OutputAsset, OutputChunk,
  PreliminaryFilename, SourceMapType,
};
use rolldown_error::BuildError;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{
  path_buf_ext::PathBufExt,
  path_ext::PathExt,
  rayon::{IntoParallelRefIterator, ParallelBridge, ParallelIterator},
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
    augment_chunk_hash::augment_chunk_hash,
    chunk::{
      deconflict_chunk_symbols::deconflict_chunk_symbols,
      finalize_chunks::finalize_chunks,
      render_chunk::{render_chunk, ChunkRenderReturn},
    },
    extract_hash_pattern::extract_hash_pattern,
    extract_meaningful_input_name_from_path::try_extract_meaningful_input_name_from_path,
    finalize_normal_module,
    hash_placeholder::HashPlaceholderGenerator,
    render_chunks::render_chunks,
  },
  BundleOutput, SharedOptions,
};

mod code_splitting;
mod compute_cross_chunk_links;

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
  pub async fn generate(&mut self) -> Result<BundleOutput> {
    let mut chunk_graph = self.generate_chunks();

    self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph)?;

    self.compute_cross_chunk_links(&mut chunk_graph);

    chunk_graph.chunks.iter_mut().par_bridge().for_each(|chunk| {
      deconflict_chunk_symbols(chunk, self.link_output);
    });

    let ast_table_iter = self.link_output.ast_table.iter_mut_enumerated();
    ast_table_iter
      .par_bridge()
      .filter(|(id, _)| {
        self.link_output.module_table.modules[*id].as_ecma().map_or(false, |m| m.is_included)
      })
      .for_each(|(id, ast)| {
        let Module::Ecma(module) = &self.link_output.module_table.modules[id] else {
          return;
        };
        let chunk_id = chunk_graph.module_to_chunk[module.idx].unwrap();
        let chunk = &chunk_graph.chunks[chunk_id];
        let linking_info = &self.link_output.metas[module.idx];
        if self.options.format.requires_scope_hoisting() {
          finalize_normal_module(
            module,
            ScopeHoistingFinalizerContext {
              canonical_names: &chunk.canonical_names,
              id: module.idx,
              symbols: &self.link_output.symbols,
              linking_info,
              module,
              modules: &self.link_output.module_table.modules,
              linking_infos: &self.link_output.metas,
              runtime: &self.link_output.runtime,
              chunk_graph: &chunk_graph,
              options: self.options,
              top_level_member_expr_resolved_cache: &self
                .link_output
                .top_level_member_expr_resolved_cache,
            },
            ast,
          );
        } else {
          ast.program.with_mut(|fields| {
            let (oxc_program, alloc) = (fields.program, fields.allocator);
            let mut finalizer = IsolatingModuleFinalizer {
              alloc,
              // scope: &module.scope,
              ctx: &IsolatingModuleFinalizerContext {
                module,
                modules: &self.link_output.module_table.modules,
              },
              snippet: AstSnippet::new(alloc),
            };
            finalizer.visit_program(oxc_program);
          });
        }
      });

    let chunks = try_join_all(
      chunk_graph
        .chunks
        .iter()
        .map(|c| async { render_chunk(c, self.options, self.link_output, &chunk_graph).await }),
    )
    .await?;

    let chunks = render_chunks(self.plugin_driver, chunks).await?;

    let chunks = augment_chunk_hash(self.plugin_driver, chunks).await?;

    let chunks = finalize_chunks(&mut chunk_graph, chunks);

    let mut assets = vec![];
    for ChunkRenderReturn {
      mut map,
      rendered_chunk,
      mut code,
      file_dir,
      preliminary_filename,
      ..
    } in chunks
    {
      if let Some(map) = map.as_mut() {
        map.set_file(&rendered_chunk.filename);

        let map_filename = format!("{}.map", rendered_chunk.filename.as_str());
        let map_path = file_dir.join(&map_filename);

        if let Some(source_map_ignore_list) = &self.options.sourcemap_ignore_list {
          let mut x_google_ignore_list = vec![];
          for (index, source) in map.get_sources().enumerate() {
            if source_map_ignore_list.call(source, map_path.to_string_lossy().as_ref()).await? {
              #[allow(clippy::cast_possible_truncation)]
              x_google_ignore_list.push(index as u32);
            }
          }
          if !x_google_ignore_list.is_empty() {
            map.set_x_google_ignore_list(x_google_ignore_list);
          }
        }

        if let Some(sourcemap_path_transform) = &self.options.sourcemap_path_transform {
          let mut sources = Vec::with_capacity(map.get_sources().count());
          for source in map.get_sources() {
            sources.push(
              sourcemap_path_transform.call(source, map_path.to_string_lossy().as_ref()).await?,
            );
          }
          map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());
        }

        // Normalize the windows path at final.
        let sources = map.get_sources().map(|x| x.to_slash_lossy().to_string()).collect::<Vec<_>>();
        map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());

        match self.options.sourcemap {
          SourceMapType::File => {
            let source = match map.to_json_string().map_err(BuildError::sourcemap_error) {
              Ok(source) => source,
              Err(e) => {
                self.link_output.errors.push(e);
                continue;
              }
            };
            assets.push(Output::Asset(Box::new(OutputAsset {
              filename: map_filename.clone(),
              source: source.into(),
            })));
            code.push_str(&format!("\n//# sourceMappingURL={map_filename}"));
          }
          SourceMapType::Inline => {
            let data_url = match map.to_data_url().map_err(BuildError::sourcemap_error) {
              Ok(data_url) => data_url,
              Err(e) => {
                self.link_output.errors.push(e);
                continue;
              }
            };
            code.push_str(&format!("\n//# sourceMappingURL={data_url}"));
          }
          SourceMapType::Hidden => {}
        }
      }
      let sourcemap_filename =
        map.as_ref().map(|_| format!("{}.map", rendered_chunk.filename.as_str()));
      assets.push(Output::Chunk(Box::new(OutputChunk {
        name: rendered_chunk.name,
        filename: rendered_chunk.filename,
        code,
        is_entry: rendered_chunk.is_entry,
        is_dynamic_entry: rendered_chunk.is_dynamic_entry,
        facade_module_id: rendered_chunk.facade_module_id,
        modules: rendered_chunk.modules,
        exports: rendered_chunk.exports,
        module_ids: rendered_chunk.module_ids,
        imports: rendered_chunk.imports,
        dynamic_imports: rendered_chunk.dynamic_imports,
        map,
        sourcemap_filename,
        preliminary_filename: preliminary_filename.to_string(),
      })));
    }

    // Make sure order of assets are deterministic
    // TODO: use `preliminary_filename` on `Output::Asset` instead
    assets.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));

    Ok(BundleOutput {
      assets,
      warnings: std::mem::take(&mut self.link_output.warnings),
      errors: std::mem::take(&mut self.link_output.errors),
    })
  }

  // Notices:
  // - Should generate filenames that are stable cross builds and os.
  // #[tracing::instrument(level = "debug", skip_all)]
  fn generate_chunk_name_and_preliminary_filenames(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> anyhow::Result<()> {
    struct ChunkNameInfo {
      pub name: String,
      pub explicit: bool,
    }

    let modules = &self.link_output.module_table.modules;

    let mut index_pre_generated_names: IndexVec<ChunkIdx, ChunkNameInfo> = chunk_graph
      .chunks
      .as_vec()
      .par_iter()
      .map(|chunk| {
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, is_user_defined, .. } => {
            if let Some(name) = &chunk.name {
              ChunkNameInfo { name: name.to_string(), explicit: true }
            } else {
              let module = &modules[entry_module_id];
              let generated = if is_user_defined {
                try_extract_meaningful_input_name_from_path(module.resource_id())
                  .unwrap_or("input".to_string())
              } else {
                module.resource_id().as_path().representative_file_name().into_owned()
              };
              ChunkNameInfo { name: generated, explicit: false }
            }
          }
          ChunkKind::Common => {
            // - rollup use the first entered/last executed module as the `[name]` of common chunks.
            // - esbuild always use 'chunk' as the `[name]`. However we try to make the name more meaningful here.
            let first_executed_non_runtime_module =
              chunk.modules.iter().rev().find(|each| **each != self.link_output.runtime.id());
            ChunkNameInfo {
              name: first_executed_non_runtime_module.map_or_else(
                || "chunk".to_string(),
                |module_id| {
                  let module = &modules[*module_id];
                  module.resource_id().as_path().representative_file_name().into_owned()
                },
              ),
              explicit: false,
            }
          }
        }
      })
      .collect::<Vec<_>>()
      .into();

    // We make entries listed first to ensure names of user-defined entries chunks aren't shadowed by other chunks
    let chunk_ids = chunk_graph
      .user_defined_entry_chunk_ids
      .iter()
      .copied()
      .chain(chunk_graph.sorted_chunk_idx_vec.iter().copied())
      .collect::<IndexSet<_>>();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();
    let mut used_names: FxHashSet<String> = FxHashSet::default();

    chunk_ids.into_iter().try_for_each(|chunk_id| -> anyhow::Result<()> {
      let chunk = &mut chunk_graph.chunks[chunk_id];
      if chunk.preliminary_filename.is_some() {
        // Already generated
        return Ok(());
      }

      let chunk_name_info = &mut index_pre_generated_names[chunk_id];

      let chunk_name = if chunk_name_info.explicit {
        if used_names.contains(&chunk_name_info.name) {
          return Err(anyhow::anyhow!("Chunk name `{}` is already used", chunk_name_info.name));
        }
        chunk_name_info.name.clone()
      } else {
        let mut chunk_name = chunk_name_info.name.clone();
        let mut next_count = 1;
        while used_names.contains(&chunk_name) {
          chunk_name = format!("{chunk_name}~{next_count}");
          next_count += 1;
        }
        chunk_name
      };

      used_names.insert(chunk_name.clone());

      let filename_template = chunk.filename_template(self.options);
      let extracted_hash_pattern = extract_hash_pattern(filename_template.template());

      let hash_placeholder =
        extracted_hash_pattern.map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));

      let preliminary = filename_template.render(&FileNameRenderOptions {
        name: Some(&chunk_name),
        hash: hash_placeholder.as_deref(),
        ..Default::default()
      });

      chunk.name = Some(chunk_name.into());
      chunk.absolute_preliminary_filename =
        Some(preliminary.absolutize_with(&self.options.dir).expect_into_string());
      chunk.preliminary_filename = Some(PreliminaryFilename::new(preliminary, hash_placeholder));
      Ok(())
    })?;

    Ok(())
  }
}
