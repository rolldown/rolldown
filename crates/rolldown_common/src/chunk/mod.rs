// cSpell:disable
use crate::{
  ChunkIdx, ChunkKind, FilenameTemplate, ModuleIdx, NamedImport, NormalizedBundlerOptions,
  RollupPreRenderedChunk, SymbolRef,
};
pub mod chunk_table;
pub mod types;

use arcstr::ArcStr;
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
  pub modules: Vec<ModuleIdx>,
  pub name: Option<ArcStr>,
  pub pre_rendered_chunk: Option<RollupPreRenderedChunk>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub absolute_preliminary_filename: Option<String>,
  pub css_preliminary_filename: Option<PreliminaryFilename>,
  pub css_absolute_preliminary_filename: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  // Sorted by Module#stable_id of modules in the chunk
  pub cross_chunk_imports: Vec<ChunkIdx>,
  pub cross_chunk_dynamic_imports: Vec<ChunkIdx>,
  pub bits: BitSet,
  pub imports_from_other_chunks: Vec<(ChunkIdx, Vec<CrossChunkImportItem>)>,
  // Only meaningful for cjs format
  pub require_binding_names_for_other_chunks: FxHashMap<ChunkIdx, String>,
  pub imports_from_external_modules: Vec<(ModuleIdx, Vec<NamedImport>)>,
  // meaningless if the chunk is an entrypoint
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Rstr>,
}

impl Chunk {
  pub fn new(name: Option<ArcStr>, bits: BitSet, modules: Vec<ModuleIdx>, kind: ChunkKind) -> Self {
    Self {
      exec_order: u32::MAX,
      modules,
      name: name.map(Into::into),
      bits,
      kind,
      ..Self::default()
    }
  }

  pub fn has_side_effect(&self, runtime_id: ModuleIdx) -> bool {
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

  pub async fn filename_template<'a>(
    &mut self,
    options: &'a NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
  ) -> anyhow::Result<FilenameTemplate> {
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined)
    {
      options.entry_filenames.call(rollup_pre_rendered_chunk).await?
    } else {
      options.chunk_filenames.call(rollup_pre_rendered_chunk).await?
    };

    Ok(FilenameTemplate::new(ret))
  }

  pub async fn css_filename_template<'a>(
    &mut self,
    options: &'a NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
  ) -> anyhow::Result<FilenameTemplate> {
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { is_user_defined, .. } if is_user_defined)
    {
      options.css_entry_filenames.call(rollup_pre_rendered_chunk).await?
    } else {
      options.css_chunk_filenames.call(rollup_pre_rendered_chunk).await?
    };

    Ok(FilenameTemplate::new(ret))
  }
}
