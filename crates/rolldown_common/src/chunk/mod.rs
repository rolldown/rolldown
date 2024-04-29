// cSpell:disable
use crate::{
  ChunkId, ChunkKind, ExternalModuleId, FilePath, FilenameTemplate, NamedImport, NormalModuleId,
  NormalizedBundlerOptions, SymbolRef,
};
pub mod types;

use rolldown_rstr::Rstr;
use rolldown_utils::BitSet;
use rustc_hash::FxHashMap;

use self::types::{
  cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
};

#[derive(Debug, Default)]
pub struct Chunk {
  pub kind: ChunkKind,
  pub modules: Vec<NormalModuleId>,
  pub name: Option<String>,
  pub filename: Option<FilePath>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  // Sorted by resource_id of modules in the chunk
  pub cross_chunk_imports: Vec<ChunkId>,
  pub cross_chunk_dynamic_imports: Vec<ChunkId>,
  pub bits: BitSet,
  pub imports_from_other_chunks: FxHashMap<ChunkId, Vec<CrossChunkImportItem>>,
  pub imports_from_external_modules: FxHashMap<ExternalModuleId, Vec<NamedImport>>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
}

impl Chunk {
  pub fn new(
    name: Option<String>,
    bits: BitSet,
    modules: Vec<NormalModuleId>,
    kind: ChunkKind,
  ) -> Self {
    Self { modules, name, bits, kind, ..Self::default() }
  }

  pub fn file_name_template<'a>(
    &mut self,
    options: &'a NormalizedBundlerOptions,
  ) -> &'a FilenameTemplate {
    if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined) {
      &options.entry_file_names
    } else {
      &options.chunk_file_names
    }
  }
}
