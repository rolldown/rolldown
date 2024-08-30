use std::collections::hash_map::Entry;

use anyhow::Result;
use arcstr::ArcStr;
use oxc::{ast::VisitMut, index::IndexVec};
use rolldown_ecmascript::AstSnippet;
use rustc_hash::FxHashMap;

use rolldown_common::{ChunkIdx, ChunkKind, FileNameRenderOptions, Module, PreliminaryFilename};
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{
  path_buf_ext::PathBufExt,
  path_ext::PathExt,
  rayon::{IntoParallelRefIterator, ParallelBridge, ParallelIterator},
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
    chunk::{deconflict_chunk_symbols::deconflict_chunk_symbols, generate_pre_rendered_chunk},
    extract_hash_pattern::extract_hash_pattern,
    extract_meaningful_input_name_from_path::try_extract_meaningful_input_name_from_path,
    finalize_normal_module,
    hash_placeholder::HashPlaceholderGenerator,
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
  pub async fn generate(&mut self) -> Result<BundleOutput> {
    let mut chunk_graph = self.generate_chunks();

    self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;

    self.compute_cross_chunk_links(&mut chunk_graph);

    chunk_graph.chunks.iter_mut().par_bridge().for_each(|chunk| {
      deconflict_chunk_symbols(chunk, self.link_output, &self.options.format);
    });

    let ast_table_iter = self.link_output.ast_table.iter_mut();
    ast_table_iter
      .par_bridge()
      .filter(|(_ast, owner)| {
        self.link_output.module_table.modules[*owner].as_ecma().map_or(false, |m| m.is_included)
      })
      .for_each(|(ast, owner)| {
        let Module::Ecma(module) = &self.link_output.module_table.modules[*owner] else {
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

    self.render_chunk_to_assets(&mut chunk_graph).await
  }

  // Notices:
  // - Should generate filenames that are stable cross builds and os.
  // #[tracing::instrument(level = "debug", skip_all)]
  #[allow(clippy::too_many_lines)]
  async fn generate_chunk_name_and_preliminary_filenames(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> anyhow::Result<()> {
    struct ChunkNameInfo {
      pub name: ArcStr,
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
              ChunkNameInfo { name: name.clone(), explicit: true }
            } else {
              let module = &modules[entry_module_id];
              let generated = if is_user_defined {
                try_extract_meaningful_input_name_from_path(module.id())
                  .map(ArcStr::from)
                  .unwrap_or(arcstr::literal!("input"))
              } else {
                ArcStr::from(sanitize_file_name(module.id().as_path().representative_file_name()))
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
                || arcstr::literal!("chunk"),
                |module_id| {
                  let module = &modules[*module_id];
                  ArcStr::from(sanitize_file_name(module.id().as_path().representative_file_name()))
                },
              ),
              explicit: false,
            }
          }
        }
      })
      .collect::<Vec<_>>()
      .into();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();
    let mut used_name_map: FxHashMap<ArcStr, u32> = FxHashMap::default();
    for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &mut chunk_graph.chunks[*chunk_id];
      if chunk.preliminary_filename.is_some() {
        // Already generated
        continue;
      }

      let chunk_name_info = &mut index_pre_generated_names[*chunk_id];

      let chunk_name = if chunk_name_info.explicit {
        if used_name_map.contains_key(&chunk_name_info.name) {
          return Err(anyhow::anyhow!("Chunk name `{}` is already used", chunk_name_info.name));
        }
        chunk_name_info.name.clone()
      } else {
        let chunk_name = match used_name_map.entry(chunk_name_info.name.clone()) {
          Entry::Occupied(mut occ) => {
            let next_count = *occ.get();
            occ.insert(next_count + 1);
            ArcStr::from(format!("{}~{next_count}", occ.key()))
          }
          Entry::Vacant(vac) => vac.key().clone(),
        };
        chunk_name
      };
      used_name_map.insert(chunk_name.clone(), 1);
      chunk.name = Some(chunk_name.clone());
      let pre_rendered_chunk = generate_pre_rendered_chunk(chunk, self.link_output, self.options);

      let filename_template = chunk.filename_template(self.options, &pre_rendered_chunk).await?;
      let css_filename_template =
        chunk.css_filename_template(self.options, &pre_rendered_chunk).await?;
      chunk.pre_rendered_chunk = Some(pre_rendered_chunk);
      let extracted_hash_pattern = extract_hash_pattern(filename_template.template());
      let extracted_css_hash_pattern = extract_hash_pattern(css_filename_template.template());

      let hash_placeholder =
        extracted_hash_pattern.map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));

      let css_hash_placeholder =
        extracted_css_hash_pattern.map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));

      let preliminary = filename_template.render(&FileNameRenderOptions {
        name: Some(&chunk_name),
        hash: hash_placeholder.as_deref(),
        ..Default::default()
      });

      let css_preliminary = css_filename_template.render(&FileNameRenderOptions {
        name: Some(&chunk_name),
        hash: hash_placeholder.as_deref(),
        ..Default::default()
      });

      chunk.absolute_preliminary_filename = Some(
        preliminary.absolutize_with(self.options.cwd.join(&self.options.dir)).expect_into_string(),
      );
      chunk.css_absolute_preliminary_filename = Some(
        css_preliminary
          .absolutize_with(self.options.cwd.join(&self.options.dir))
          .expect_into_string(),
      );
      chunk.preliminary_filename = Some(PreliminaryFilename::new(preliminary, hash_placeholder));
      chunk.css_preliminary_filename =
        Some(PreliminaryFilename::new(css_preliminary, css_hash_placeholder));
    }
    Ok(())
  }
}
