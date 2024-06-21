use std::sync::Arc;

// cSpell:disable
use crate::{
  ChunkId, ChunkKind, ExternalModuleId, FilenameTemplate, NamedImport, NormalModuleId,
  NormalizedBundlerOptions, ResourceId, SymbolRef,
};
pub mod types;

use rolldown_rstr::Rstr;
use rolldown_utils::{path_ext::PathExt, BitSet};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use self::types::{
  cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
};

#[derive(Debug, Default)]
pub struct Chunk {
  pub exec_order: u32,
  pub kind: ChunkKind,
  pub modules: Vec<NormalModuleId>,
  pub user_defined_name: Option<String>,
  pub filename: Option<ResourceId>,
  pub name: Option<Arc<str>>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub absolute_preliminary_filename: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  // Sorted by resource_id of modules in the chunk
  pub cross_chunk_imports: Vec<ChunkId>,
  pub cross_chunk_dynamic_imports: Vec<ChunkId>,
  pub bits: BitSet,
  pub imports_from_other_chunks: Vec<(ChunkId, Vec<CrossChunkImportItem>)>,
  pub imports_from_external_modules: Vec<(ExternalModuleId, Vec<NamedImport>)>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
}

impl Chunk {
  pub fn new(
    user_defined_name: Option<String>,
    bits: BitSet,
    modules: Vec<NormalModuleId>,
    kind: ChunkKind,
  ) -> Self {
    Self { exec_order: u32::MAX, modules, user_defined_name, bits, kind, ..Self::default() }
  }

  pub fn filename_template<'a>(
    &mut self,
    options: &'a NormalizedBundlerOptions,
  ) -> &'a FilenameTemplate {
    if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined) {
      &options.entry_filenames
    } else {
      &options.chunk_filenames
    }
  }

  pub fn has_side_effect(&self, runtime_id: NormalModuleId) -> bool {
    // TODO: remove this special case, once `NormalModule#side_effect` is implemented. Runtime module should always not have side effect
    if self.modules.len() == 1 && self.modules[0] == runtime_id {
      return false;
    }
    // TODO: Wether a chunk has side effect is determined by wether it's module has side effect
    // Now we just return `true`
    true
  }

  pub fn import_path_for(&self, importee: &Chunk) -> String {
    let importer_dir =
      self.absolute_preliminary_filename.as_ref().unwrap().as_path().parent().unwrap();
    let importee_filename = importee.absolute_preliminary_filename.as_ref().unwrap();
    let import_path = importee_filename.relative(importer_dir).as_path().expect_to_slash();

    if import_path.starts_with('.') {
      import_path
    } else {
      format!("./{import_path}")
    }
  }
}
