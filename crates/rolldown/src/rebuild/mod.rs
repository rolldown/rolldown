use rolldown_common::{
  EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, Output, PreliminaryFilename,
};
use rustc_hash::FxHashMap;

use crate::{type_alias::IndexInstantiatedChunks, SharedOptions};

#[derive(Debug, Default)]
pub struct RebuildManager {
  pub enabled: bool,
  pub old_outputs: Vec<Output>,
}

impl RebuildManager {
  pub fn new(enabled: bool) -> Self {
    Self { enabled, old_outputs: vec![] }
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
    let mut changed_modules: Vec<(ModuleId, Option<String>)> = vec![];
    for output in &self.old_outputs {
      match output {
        rolldown_common::Output::Chunk(old_chunk) => {
          for chunk in instantiated_chunks.iter() {
            match &chunk.kind {
              InstantiationKind::Ecma(ecma) => {
                let new_chunk = &ecma.rendered_chunk;
                if new_chunk.name == old_chunk.name {
                  for (module_id, module) in &new_chunk.modules {
                    // TODO: compare by hash
                    let module_code = module.code();
                    let is_new = match old_chunk.modules.get(module_id) {
                      Some(old_module) => module_code != old_module.code(),
                      None => true,
                    };
                    if is_new {
                      changed_modules.push((module_id.clone(), module_code));
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
      // TODO: should reuse EcmaGenerator::instantiate_chunk?
      let content =
        changed_modules.into_iter().filter_map(|(_, code)| code).collect::<Vec<_>>().join("\n");
      instantiated_chunks.push(InstantiatedChunk {
        origin_chunk: 0.into(),
        content: content.into(),
        map: None,
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
        file_dir: options.cwd.as_path().join(&options.dir),
        preliminary_filename: PreliminaryFilename::new("hmr-update.js".into(), None),
      });
    }
  }
}
