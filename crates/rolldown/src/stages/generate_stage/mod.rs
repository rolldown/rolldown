use std::{path::PathBuf, sync::Arc};

use arcstr::ArcStr;
use futures::future::try_join_all;
use oxc_index::IndexVec;
use render_chunk_to_assets::set_emitted_chunk_preliminary_filenames;
use rolldown_common::{
  ChunkIdx, ChunkKind, ImportMetaRolldownAssetReplacer, Module, OutputExports, PreliminaryFilename,
  RollupPreRenderedAsset,
};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::OptionExt as _;
use rolldown_std_utils::{
  PathBufExt as _, PathExt as _, representative_file_name_for_preserve_modules,
};
use rolldown_utils::{
  dashmap::FxDashMap,
  hash_placeholder::HashPlaceholderGenerator,
  make_unique_name::make_unique_name,
  rayon::{IntoParallelRefMutIterator as _, ParallelIterator as _},
};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath as _;
use tracing::debug_span;

const COMMON_JS_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

#[derive(Debug)]
struct PreGeneratedChunkName {
  /// The representative name used for symbol deconflicting and chunk binding references.
  representative_chunk_name: ArcStr,
  /// The full chunk name including directory structure relative to `preserveModulesRoot`.
  /// This appears in `PreRenderedChunk.name` and hooks like `entryFileNames`.
  chunk_name: ArcStr,
  /// The base filename for generating preliminary filenames.
  /// Absolute path without extension, used as input to filename templates.
  chunk_filename: ArcStr,
}

use crate::{
  BundleOutput, SharedOptions,
  chunk_graph::ChunkGraph,
  stages::link_stage::LinkStageOutput,
  types::generator::GenerateContext,
  utils::chunk::{
    deconflict_chunk_symbols::deconflict_chunk_symbols,
    determine_export_mode::determine_export_mode, generate_pre_rendered_chunk,
    render_chunk_exports::get_chunk_export_names,
    validate_options_for_multi_chunk_output::validate_options_for_multi_chunk_output,
  },
};

mod chunk_ext;
mod chunk_optimizer;
mod code_splitting;
mod compute_cross_chunk_links;
mod detect_ineffective_dynamic_imports;
mod finalize_modules;
mod manual_code_splitting;
mod minify_chunks;
mod on_demand_wrapping;
mod post_banner_footer;
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

    self.finalized_module_namespace_ref_usage();

    self.compute_cross_chunk_links(&mut chunk_graph);

    self.ensure_lazy_module_initialization_order(&mut chunk_graph);

    self.on_demand_wrapping(&mut chunk_graph);

    self.merge_cjs_namespace(&mut chunk_graph);

    self.trace_action_chunks_infos(&chunk_graph);

    let mut warnings = vec![];
    self.compute_chunk_output_exports(&mut chunk_graph, &mut warnings)?;
    if !warnings.is_empty() {
      self.link_output.warnings.extend(warnings);
    }

    let index_chunk_id_to_name =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;
    self.patch_asset_modules(&chunk_graph);
    set_emitted_chunk_preliminary_filenames(&self.plugin_driver.file_emitter, &chunk_graph);

    debug_span!("deconflict_chunk_symbols").in_scope(|| {
      chunk_graph.chunk_table.par_iter_mut().for_each(|chunk| {
        deconflict_chunk_symbols(
          chunk,
          self.link_output,
          self.options.format,
          &index_chunk_id_to_name,
        );
      });
    });

    self.finalize_modules(&mut chunk_graph);
    self.detect_ineffective_dynamic_imports(&chunk_graph);
    self.render_chunk_to_assets(&chunk_graph).await
  }

  /// Notices:
  /// - Should generate filenames that are stable cross builds and os.
  #[tracing::instrument(level = "debug", skip_all)]
  async fn generate_chunk_name_and_preliminary_filenames(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<FxHashMap<ChunkIdx, ArcStr>> {
    let modules = &self.link_output.module_table.modules;

    let mut index_chunk_id_to_representative_name = FxHashMap::default();

    let index_pre_generated_names_futures = chunk_graph.chunk_table.iter().map(|chunk| {
      let sanitize_filename = self.options.sanitize_filename.clone();
      let preserve_modules_root = self.options.preserve_modules_root.clone();
      let input_base = chunk.input_base.clone();
      let virtual_dirname = self.options.virtual_dirname.clone();
      async move {
        if let Some(name) = &chunk.name {
          let name = sanitize_filename.call(name).await?;
          return anyhow::Ok(PreGeneratedChunkName {
            representative_chunk_name: name.clone(),
            chunk_name: name.clone(),
            chunk_filename: name,
          });
        }
        match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, meta, .. } => {
            let module = &modules[entry_module_id];
            let generated = if self.options.preserve_modules {
              let module_id = module.id().as_str();
              let (representative_chunk_name, absolute_chunk_file_name, ext) =
                representative_file_name_for_preserve_modules(module_id.as_path());

              let sanitized_absolute_filename =
                sanitize_filename.call(absolute_chunk_file_name.as_str()).await?;

              // Apply the same logic as get_preserve_modules_chunk_name to include directory structure
              let chunk_name = {
                let p = PathBuf::from(sanitized_absolute_filename.as_str());
                let relative_path = if p.is_absolute() {
                  if let Some(ref preserve_modules_root) = preserve_modules_root {
                    if absolute_chunk_file_name.starts_with(preserve_modules_root.as_str()) {
                      absolute_chunk_file_name[preserve_modules_root.len()..]
                        .trim_start_matches(['/', '\\'])
                        .to_string()
                    } else {
                      p.relative(input_base.as_str()).to_slash_lossy().into_owned()
                    }
                  } else {
                    p.relative(input_base.as_str()).to_slash_lossy().into_owned()
                  }
                } else {
                  PathBuf::from(virtual_dirname.as_str()).join(p).to_slash_lossy().into_owned()
                };
                // `p` may be an absolute or relative path without extension, depending on the module path.
                // Now we need to add the extension back when generating the relative chunk name.
                // skip some common extension https://github.com/rollup/rollup/pull/4565/files
                match ext.as_deref() {
                  Some(e) if COMMON_JS_EXTENSIONS.contains(&e) => relative_path,
                  Some(e) if !e.is_empty() => format!("{relative_path}.{e}"),
                  _ => relative_path,
                }
              };

              let sanitized_representative_chunk_name =
                sanitize_filename.call(&representative_chunk_name).await?;
              PreGeneratedChunkName {
                representative_chunk_name: sanitized_representative_chunk_name,
                chunk_name: chunk_name.into(),
                chunk_filename: sanitized_absolute_filename,
              }
            } else if meta.contains(rolldown_common::ChunkMeta::UserDefinedEntry) {
              // try extract meaningful input name from path
              if let Some(file_stem) =
                module.id().as_str().as_path().file_stem().and_then(|f| f.to_str())
              {
                let name = sanitize_filename.call(file_stem).await?;
                PreGeneratedChunkName {
                  chunk_name: name.clone(),
                  representative_chunk_name: name.clone(),
                  chunk_filename: name,
                }
              } else {
                let name = arcstr::literal!("input");
                PreGeneratedChunkName {
                  representative_chunk_name: name.clone(),
                  chunk_name: name.clone(),
                  chunk_filename: name,
                }
              }
            } else {
              let chunk_name = sanitize_filename
                .call(&module.id().as_str().as_path().representative_file_name())
                .await?;

              PreGeneratedChunkName {
                representative_chunk_name: chunk_name.clone(),
                chunk_name: chunk_name.clone(),
                chunk_filename: chunk_name,
              }
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
              let module_id = module.id().as_str();
              let name = module_id.as_path().representative_file_name();
              let sanitized_filename = sanitize_filename.call(&name).await?;
              Ok(PreGeneratedChunkName {
                representative_chunk_name: sanitized_filename.clone(),
                chunk_name: sanitized_filename.clone(),
                chunk_filename: sanitized_filename,
              })
            } else {
              let name = arcstr::literal!("chunk");
              Ok(PreGeneratedChunkName {
                representative_chunk_name: name.clone(),
                chunk_name: name.clone(),
                chunk_filename: name,
              })
            }
          }
        }
      }
    });

    let mut index_pre_generated_names: IndexVec<ChunkIdx, PreGeneratedChunkName> =
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
      index_chunk_id_to_representative_name
        .insert(*chunk_id, pre_generated_chunk_name.representative_chunk_name.clone());
      let pre_rendered_chunk =
        generate_pre_rendered_chunk(chunk, &pre_generated_chunk_name.chunk_name, self.link_output);

      let preliminary_filename = chunk
        .generate_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          &pre_generated_chunk_name.chunk_filename,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;

      // Defer chunk name assignment to make sure at this point only entry chunk have a name
      // if user provided one.
      chunk.name = Some(pre_generated_chunk_name.chunk_name.clone());

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
              source: asset_view.source.clone(),
            })
            .await?;

          let has_hash_pattern = asset_filename_template.has_hash_pattern();
          let extension = module.id.as_path().extension().and_then(|s| s.to_str());

          let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
          let hash_replacer = has_hash_pattern.then(|| {
            let pattern_name = asset_filename_template.pattern_name();
            |len: Option<usize>| {
              let hash = hash_placeholder_generator.generate(len, pattern_name)?;
              if let Some(hash_placeholder) = hash_placeholder.as_mut() {
                hash_placeholder.push(hash.clone());
              }
              Ok(hash)
            }
          });

          let mut filename =
            asset_filename_template.render(Some(&name), None, extension, hash_replacer)?.into();
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
      chunk.preliminary_filename = Some(preliminary_filename);
    }
    Ok(index_chunk_id_to_representative_name)
  }

  pub fn patch_asset_modules(&mut self, chunk_graph: &ChunkGraph) {
    chunk_graph.chunk_table.iter().for_each(|chunk| {
      // replace asset name in ecma view
      chunk.asset_preliminary_filenames.iter().for_each(|(module_idx, preliminary)| {
        let Module::Normal(module) = &mut self.link_output.module_table[*module_idx] else {
          return;
        };
        let asset_filename: ArcStr = preliminary.as_str().into();
        module
          .ecma_view
          .mutations
          .push(Arc::new(ImportMetaRolldownAssetReplacer { asset_filename }));
      });
    });
  }

  fn compute_chunk_output_exports(
    &self,
    chunk_graph: &mut ChunkGraph,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<()> {
    // Collect all the chunk data we need first
    let mut chunk_export_data = Vec::new();
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if let Some(entry_module) = chunk.user_defined_entry_module(&self.link_output.module_table) {
        let export_names = get_chunk_export_names(chunk, self.link_output);
        chunk_export_data.push((chunk_idx, entry_module, export_names));
      }
    }

    // Now compute the export modes for each chunk
    for (chunk_idx, entry_module, export_names) in chunk_export_data {
      let export_mode = determine_export_mode(
        warnings,
        &GenerateContext {
          chunk: &chunk_graph.chunk_table[chunk_idx],
          options: self.options,
          link_output: self.link_output,
          chunk_graph,
          plugin_driver: self.plugin_driver,
          module_id_to_codegen_ret: Vec::new(),
          render_export_items_index_vec: &IndexVec::default(),
          chunk_idx,
        },
        entry_module,
        &export_names,
      )?;
      chunk_graph.chunk_table[chunk_idx].output_exports = export_mode;
    }

    // Set common chunks to Named
    for chunk in chunk_graph.chunk_table.iter_mut() {
      if chunk.user_defined_entry_module(&self.link_output.module_table).is_none() {
        chunk.output_exports = OutputExports::Named;
      }
    }

    Ok(())
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
