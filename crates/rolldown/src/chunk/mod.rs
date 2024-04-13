// cSpell:disable
use index_vec::IndexVec;
use rolldown_common::{ChunkId, FileNameTemplate, NormalizedBundlerOptions};
pub mod types;

pub type ChunksVec = IndexVec<ChunkId, Chunk>;

use rolldown_common::{
  ChunkKind, ExternalModuleId, NamedImport, NormalModuleId, RenderedChunk, Specifier, SymbolRef,
};
use rolldown_rstr::Rstr;
use rolldown_sourcemap::SourceMap;
use rolldown_utils::BitSet;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct CrossChunkImportItem {
  pub export_alias: Option<Specifier>,
  pub import_ref: SymbolRef,
}

#[derive(Debug, Default)]
pub struct Chunk {
  pub kind: ChunkKind,
  pub modules: Vec<NormalModuleId>,
  pub name: Option<String>,
  pub filename: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  pub bits: BitSet,
  pub imports_from_other_chunks: FxHashMap<ChunkId, Vec<CrossChunkImportItem>>,
  pub imports_from_external_modules: FxHashMap<ExternalModuleId, Vec<NamedImport>>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
}

pub struct ChunkRenderReturn {
  pub code: String,
  pub map: Option<SourceMap>,
  pub rendered_chunk: RenderedChunk,
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
    output_options: &'a NormalizedBundlerOptions,
  ) -> &'a FileNameTemplate {
    if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined) {
      &output_options.entry_file_names
    } else {
      &output_options.chunk_file_names
    }
  }
}
