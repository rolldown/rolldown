use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

// cSpell:disable
use crate::{
  ChunkIdx, ChunkKind, FileNameRenderOptions, FilenameTemplate, ModuleIdx, ModuleTable,
  NamedImport, NormalModule, NormalizedBundlerOptions, RollupPreRenderedChunk, SymbolNameRefToken,
  SymbolRef,
};
pub mod chunk_table;
pub mod types;

use arcstr::ArcStr;
use rolldown_rstr::Rstr;
use rolldown_std_utils::PathExt;
use rolldown_utils::{
  extract_hash_pattern::extract_hash_pattern, hash_placeholder::HashPlaceholderGenerator,
  indexmap::FxIndexMap, BitSet,
};
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
  // emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  // emitted chunk corresponding reference_id, used to `PluginContext#getFileName` to search the emitted chunk name
  pub reference_id: Option<ArcStr>,
  pub pre_rendered_chunk: Option<RollupPreRenderedChunk>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub absolute_preliminary_filename: Option<String>,
  pub css_preliminary_filename: Option<PreliminaryFilename>,
  pub css_absolute_preliminary_filename: Option<String>,
  pub asset_preliminary_filenames: FxIndexMap<ModuleIdx, PreliminaryFilename>,
  pub asset_absolute_preliminary_filenames: FxIndexMap<ModuleIdx, String>,
  pub canonical_names: FxHashMap<SymbolRef, Rstr>,
  pub canonical_name_by_token: FxHashMap<SymbolNameRefToken, Rstr>,
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
  pub fn new(
    name: Option<ArcStr>,
    reference_id: Option<ArcStr>,
    file_name: Option<ArcStr>,
    bits: BitSet,
    modules: Vec<ModuleIdx>,
    kind: ChunkKind,
  ) -> Self {
    Self {
      exec_order: u32::MAX,
      modules,
      name,
      file_name,
      reference_id,
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
    let importee_filename = importee
      .absolute_preliminary_filename
      .as_ref()
      .expect("importee chunk should have absolute_preliminary_filename");
    let import_path = self.relative_path_for(importee_filename.as_path());
    if import_path.starts_with('.') {
      import_path
    } else {
      format!("./{import_path}")
    }
  }

  pub fn relative_path_for(&self, target: &Path) -> String {
    let sorurce_dir = self
      .absolute_preliminary_filename
      .as_ref()
      .expect("chunk should have absolute_preliminary_filename")
      .as_path()
      .parent()
      .expect("absolute_preliminary_filename should have a parent directory");
    target.relative(sorurce_dir).as_path().expect_to_slash()
  }

  pub async fn filename_template(
    &self,
    options: &NormalizedBundlerOptions,
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

  pub async fn css_filename_template(
    &self,
    options: &NormalizedBundlerOptions,
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

  pub async fn generate_preliminary_filename(
    &mut self,
    options: &NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
    chunk_name: &ArcStr,
    hash_placeholder_generator: &mut HashPlaceholderGenerator,
    make_unique_name: &mut impl FnMut(&ArcStr) -> ArcStr,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      let basename = PathBuf::from(file)
        .file_name()
        .expect("The file should have basename")
        .to_string_lossy()
        .to_string();
      return Ok(PreliminaryFilename::new(basename, None));
    }
    if let Some(file_name) = &self.file_name {
      return Ok(PreliminaryFilename::new(file_name.to_string(), None));
    }
    let filename_template = self.filename_template(options, rollup_pre_rendered_chunk).await?;
    let extracted_hash_pattern = extract_hash_pattern(filename_template.template());

    let hash_placeholder =
      extracted_hash_pattern.map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));

    let name = if hash_placeholder.is_some() {
      make_unique_name(chunk_name);
      Cow::Borrowed(chunk_name)
    } else {
      let unique = make_unique_name(chunk_name);
      Cow::Owned(unique)
    };

    let rendered = filename_template.render(&FileNameRenderOptions {
      name: Some(&name),
      hash: hash_placeholder.as_deref(),
      ..Default::default()
    });

    Ok(PreliminaryFilename::new(rendered, hash_placeholder))
  }

  pub async fn generate_css_preliminary_filename(
    &self,
    options: &NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
    chunk_name: &ArcStr,
    hash_placeholder_generator: &mut HashPlaceholderGenerator,
    make_unique_name: &mut impl FnMut(&ArcStr) -> ArcStr,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      let mut file = PathBuf::from(file);
      file.set_extension("css");
      return Ok(PreliminaryFilename::new(file.into_os_string().into_string().unwrap(), None));
    }
    let filename_template = self.css_filename_template(options, rollup_pre_rendered_chunk).await?;

    let extracted_hash_pattern = extract_hash_pattern(filename_template.template());

    let hash_placeholder =
      extracted_hash_pattern.map(|p| hash_placeholder_generator.generate(p.len.unwrap_or(8)));

    let name = if hash_placeholder.is_some() {
      make_unique_name(chunk_name);
      Cow::Borrowed(chunk_name)
    } else {
      let unique = make_unique_name(chunk_name);
      Cow::Owned(unique)
    };
    let rendered = filename_template.render(&FileNameRenderOptions {
      name: Some(&name),
      hash: hash_placeholder.as_deref(),
      ..Default::default()
    });

    Ok(PreliminaryFilename::new(rendered, hash_placeholder))
  }

  pub fn user_defined_entry_module_idx(&self) -> Option<ModuleIdx> {
    match &self.kind {
      ChunkKind::EntryPoint { module, is_user_defined, .. } if *is_user_defined => Some(*module),
      _ => None,
    }
  }

  pub fn user_defined_entry_module<'module>(
    &self,
    module_table: &'module ModuleTable,
  ) -> Option<&'module NormalModule> {
    self.user_defined_entry_module_idx().and_then(|idx| module_table.modules[idx].as_normal())
  }

  pub fn entry_module_idx(&self) -> Option<ModuleIdx> {
    match &self.kind {
      ChunkKind::EntryPoint { module, .. } => Some(*module),
      ChunkKind::Common => None,
    }
  }

  pub fn entry_module<'module>(
    &self,
    module_table: &'module ModuleTable,
  ) -> Option<&'module NormalModule> {
    self.entry_module_idx().and_then(|idx| module_table.modules[idx].as_normal())
  }
}
