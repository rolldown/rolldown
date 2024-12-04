use rolldown_common::{
  EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, Output, PreliminaryFilename,
  RenderedModule,
};
use rolldown_sourcemap::SourceJoiner;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use crate::{type_alias::IndexInstantiatedChunks, BundleOutput, SharedOptions};

#[derive(Debug, Default)]
pub struct RebuildManager {
  pub enabled: bool,
  old_outputs: Vec<Output>,
}

impl RebuildManager {
  pub fn save_output(&mut self, output: &BundleOutput) {
    if self.enabled {
      self.old_outputs.clone_from(&output.assets);
    }
  }

  pub fn render_hmr_chunk(
    &self,
    instantiated_chunks: &mut IndexInstantiatedChunks,
    options: &SharedOptions,
  ) {
    if !self.enabled {
      return;
    }

    // find changed modules
    // TODO: handle removed modules
    let mut changed_modules: Vec<(ModuleId, RenderedModule)> = vec![];
    for output in &self.old_outputs {
      match output {
        rolldown_common::Output::Chunk(old_chunk) => {
          for chunk in instantiated_chunks.iter() {
            match &chunk.kind {
              InstantiationKind::Ecma(ecma) => {
                let new_chunk = &ecma.rendered_chunk;
                if new_chunk.name == old_chunk.name {
                  for (module_id, module) in &new_chunk.modules {
                    // NOTE: if plugin mutates code during renderChunk, such mutation cannot be
                    // detected. In this case, plugin should make sure the change to be reflected
                    // on build hook stage (i.e. `transform/load`) for relevant modules. For example,
                    // `__VITE_ASSET_(referenceId)_` encodes asset content in `referenceId`, thus
                    // asset change is properly picked up for hmr.
                    let is_new = match old_chunk.modules.get(module_id) {
                      Some(old_module) => module.hash() != old_module.hash(),
                      None => true,
                    };
                    if is_new {
                      changed_modules.push((module_id.clone(), module.clone()));
                    }
                  }
                }
              }
              InstantiationKind::None => {}
            }
          }
        }
        rolldown_common::Output::Asset(_) => {}
      }
    }

    if !changed_modules.is_empty() {
      // create hmr chunk
      let mut source_joiner = SourceJoiner::default();
      for (_, module) in &changed_modules {
        for source in module.iter_sources() {
          source_joiner.append_source(source);
        }
      }
      let (content, mut map) = source_joiner.join();
      let file_dir = options.cwd.as_path().join(&options.dir);
      // normalize sources (same as EcmaGenerator.instantiate_chunk)
      if let Some(map) = map.as_mut() {
        let paths =
          map.get_sources().map(|source| source.as_path().relative(&file_dir)).collect::<Vec<_>>();
        let sources = paths.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>();
        map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());
      }
      instantiated_chunks.push(InstantiatedChunk {
        origin_chunk: 0.into(),
        content: content.into(),
        map,
        kind: InstantiationKind::from(EcmaAssetMeta {
          rendered_chunk: rolldown_common::RollupRenderedChunk {
            name: "hmr-update".into(),
            is_entry: false,
            is_dynamic_entry: false,
            facade_module_id: None,
            module_ids: vec![],
            exports: vec![],
            filename: "hmr-update.js".into(),
            modules: FxHashMap::default(),
            imports: vec![],
            dynamic_imports: vec![],
            debug_id: 0,
          },
        }),
        augment_chunk_hash: None,
        file_dir,
        preliminary_filename: PreliminaryFilename::new("hmr-update.js".into(), None),
      });
    }
  }
}
