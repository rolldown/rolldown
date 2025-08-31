use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use crate::{
  ChunkIdx, ChunkKind, FilenameTemplate, ImportRecordIdx, ModuleIdx, ModuleTable, NamedImport,
  NormalModule, NormalizedBundlerOptions, PreserveEntrySignatures, RenderedConcatenatedModuleParts,
  RollupPreRenderedChunk, RuntimeHelper, SymbolRef,
  chunk::types::{chunk_reason_type::ChunkReasonType, module_group::ModuleGroup},
};
pub mod chunk_table;
pub mod types;

use arcstr::ArcStr;
use oxc::span::CompactStr;
use rolldown_std_utils::PathExt;
use rolldown_utils::{
  BitSet,
  dashmap::FxDashMap,
  hash_placeholder::HashPlaceholderGenerator,
  indexmap::{FxIndexMap, FxIndexSet},
  make_unique_name::make_unique_name,
};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

use self::types::{
  cross_chunk_import_item::CrossChunkImportItem, preliminary_filename::PreliminaryFilename,
};

bitflags::bitflags! {
  #[derive(Debug, Clone, Copy, Default)]
    pub struct ChunkMeta: u8 {
        /// `true` if the chunk is dynamic imported by other modules, it would be treated as a dynamic entry if it
        /// is not a user defined entry point.
        const DynamicImported = 1;
        const UserDefinedEntry = 1 << 1;
    }
}
#[derive(Debug, Default)]
pub struct Chunk {
  pub exec_order: u32,
  pub kind: ChunkKind,
  pub modules: Vec<ModuleIdx>,
  pub name: Option<ArcStr>,
  // emitted chunk specified filename, used to generate chunk filename
  pub file_name: Option<ArcStr>,
  // emitted chunk corresponding reference_id, used to `PluginContext#getFileName` to search the emitted chunk name
  pub pre_rendered_chunk: Option<RollupPreRenderedChunk>,
  pub preliminary_filename: Option<PreliminaryFilename>,
  pub absolute_preliminary_filename: Option<String>,
  pub css_preliminary_filename: Option<PreliminaryFilename>,
  pub css_absolute_preliminary_filename: Option<String>,
  pub asset_preliminary_filenames: FxIndexMap<ModuleIdx, PreliminaryFilename>,
  pub asset_absolute_preliminary_filenames: FxIndexMap<ModuleIdx, String>,
  pub canonical_names: FxHashMap<SymbolRef, CompactStr>,
  // Sorted by Module#stable_id of modules in the chunk
  pub cross_chunk_imports: Vec<ChunkIdx>,
  pub cross_chunk_dynamic_imports: Vec<ChunkIdx>,
  pub bits: BitSet,
  pub imports_from_other_chunks: Vec<(ChunkIdx, Vec<CrossChunkImportItem>)>,
  // Only meaningful for cjs format
  pub require_binding_names_for_other_chunks: FxHashMap<ChunkIdx, String>,
  /// The first element of tuple is module idx of external module
  /// the second element is the related named import of external module.
  /// The second `ModuleIdx` is the importer of the named import which is used to look up related
  /// import attribute.
  pub direct_imports_from_external_modules: Vec<(ModuleIdx, Vec<(ModuleIdx, NamedImport)>)>,
  /// Used for cjs, umd, iife
  /// The module directly imported symbol actually came from external modules.
  pub import_symbol_from_external_modules: FxIndexSet<ModuleIdx>,
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Vec<CompactStr>>,
  pub input_base: ArcStr,
  pub create_reasons: Vec<String>,
  pub chunk_reason_type: Box<ChunkReasonType>,
  pub preserve_entry_signature: Option<PreserveEntrySignatures>,
  pub depended_runtime_helper: RuntimeHelper,
  /// related to [`crate::types::import_record::ImportRecordMeta::EntryLevelExternal`]
  pub entry_level_external_module_idx: Vec<ModuleIdx>,
  pub insert_map: FxHashMap<ModuleIdx, Vec<(ModuleIdx, ImportRecordIdx)>>,
  pub remove_map: FxHashMap<ModuleIdx, Vec<ImportRecordIdx>>,
  pub transformed_parts_rendered: FxIndexMap<(ModuleIdx, ImportRecordIdx), String>,
  pub module_groups: Vec<ModuleGroup>,
  pub module_idx_to_group_idx: FxHashMap<ModuleIdx, usize>,
  pub module_idx_to_render_concatenated_module:
    FxHashMap<ModuleIdx, RenderedConcatenatedModuleParts>,
}

impl Chunk {
  pub fn new(
    name: Option<ArcStr>,
    file_name: Option<ArcStr>,
    bits: BitSet,
    modules: Vec<ModuleIdx>,
    kind: ChunkKind,
    input_base: ArcStr,
    preserve_entry_signature: Option<PreserveEntrySignatures>,
  ) -> Self {
    Self {
      exec_order: u32::MAX,
      modules,
      name,
      file_name,
      bits,
      kind,
      input_base,
      preserve_entry_signature,
      ..Self::default()
    }
  }

  pub fn has_side_effect(&self, runtime_id: ModuleIdx) -> bool {
    // TODO: remove this special case, once `NormalModule#side_effect` is implemented. Runtime module should always not have side effect
    if self.modules.len() == 1 && self.modules[0] == runtime_id {
      return false;
    }
    // TODO: Whether a chunk has side effect is determined by whether it's module has side effect
    // Now we just return `true`
    true
  }

  pub fn import_path_for(&self, importee: &Chunk) -> String {
    let importee_filename = importee
      .absolute_preliminary_filename
      .as_ref()
      .expect("importee chunk should have absolute_preliminary_filename");
    let import_path = self.relative_path_for(importee_filename.as_path());
    if import_path.starts_with("../") { import_path } else { format!("./{import_path}") }
  }

  pub fn relative_path_for(&self, target: &Path) -> String {
    let source_dir = self
      .absolute_preliminary_filename
      .as_ref()
      .expect("chunk should have absolute_preliminary_filename")
      .as_path()
      .parent()
      .expect("absolute_preliminary_filename should have a parent directory");
    target.relative(source_dir).as_path().expect_to_slash()
  }

  pub async fn filename_template(
    &self,
    options: &NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
  ) -> anyhow::Result<FilenameTemplate> {
    // https://github.com/rollup/rollup/blob/061a0387c8654222620f602471d66afd3c582048/src/Chunk.ts?plain=1#L526-L529
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry))
      || options.preserve_modules
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
    let ret = if matches!(self.kind, ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry))
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
    used_name_counts: &FxDashMap<ArcStr, u32>,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      let basename = PathBuf::from(file)
        .file_name()
        .expect("The file should have basename")
        .to_string_lossy()
        .to_string();
      return Ok(PreliminaryFilename::new(basename.into(), None));
    }
    if let Some(file_name) = &self.file_name {
      return Ok(PreliminaryFilename::new(file_name.clone(), None));
    }

    let filename_template = self.filename_template(options, rollup_pre_rendered_chunk).await?;
    let has_hash_pattern = filename_template.has_hash_pattern();

    let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
    let hash_replacer = has_hash_pattern.then_some({
      |len: Option<usize>| {
        let hash = hash_placeholder_generator.generate(len);
        if let Some(hash_placeholder) = hash_placeholder.as_mut() {
          hash_placeholder.push(hash.clone());
        }
        hash
      }
    });
    let chunk_name = self.get_preserve_modules_chunk_name(options, chunk_name.as_str());

    let filename = filename_template.render(Some(&chunk_name), None, hash_replacer).into();

    let name = make_unique_name(&filename, used_name_counts);

    Ok(PreliminaryFilename::new(name, hash_placeholder))
  }

  fn get_preserve_modules_chunk_name<'a, 'b: 'a>(
    &'b self,
    options: &NormalizedBundlerOptions,
    chunk_name: &'a str,
  ) -> Cow<'a, str> {
    if !options.preserve_modules {
      return Cow::Borrowed(chunk_name);
    }

    // https://github.com/rollup/rollup/blob/99d4bee3277b96b30e871fb471f6c7ed55f94850/src/Chunk.ts?plain=1#L1125-L1126
    // TODO: We need to add `ChunkNames` to module struct
    if let Some(ref name) = self.name {
      return Cow::Borrowed(name.as_str());
    }

    let p = PathBuf::from(chunk_name);
    let p = if p.is_absolute() {
      if let Some(ref preserve_modules_root) = options.preserve_modules_root {
        if chunk_name.starts_with(preserve_modules_root) {
          return Cow::Borrowed(
            chunk_name[preserve_modules_root.len()..].trim_start_matches(['/', '\\']),
          );
        }
      }
      p.relative(self.input_base.as_str())
    } else {
      PathBuf::from(&options.virtual_dirname).join(p)
    };
    Cow::Owned(p.to_slash_lossy().into_owned())
  }

  pub async fn generate_css_preliminary_filename(
    &self,
    options: &NormalizedBundlerOptions,
    rollup_pre_rendered_chunk: &RollupPreRenderedChunk,
    chunk_name: &ArcStr,
    hash_placeholder_generator: &mut HashPlaceholderGenerator,
    used_name_counts: &FxDashMap<ArcStr, u32>,
  ) -> anyhow::Result<PreliminaryFilename> {
    if let Some(file) = &options.file {
      let mut file = PathBuf::from(file);
      file.set_extension("css");
      return Ok(PreliminaryFilename::new(
        file.into_os_string().into_string().unwrap().into(),
        None,
      ));
    }

    let filename_template = self.css_filename_template(options, rollup_pre_rendered_chunk).await?;
    let has_hash_pattern = filename_template.has_hash_pattern();

    let mut hash_placeholder = has_hash_pattern.then_some(vec![]);
    let hash_replacer = has_hash_pattern.then_some({
      |len: Option<usize>| {
        let hash = hash_placeholder_generator.generate(len);
        if let Some(hash_placeholder) = hash_placeholder.as_mut() {
          hash_placeholder.push(hash.clone());
        }
        hash
      }
    });
    let chunk_name = self.get_preserve_modules_chunk_name(options, chunk_name.as_str());

    let filename = filename_template.render(Some(&chunk_name), None, hash_replacer).into();

    let name = make_unique_name(&filename, used_name_counts);

    Ok(PreliminaryFilename::new(name, hash_placeholder))
  }

  pub fn user_defined_entry_module_idx(&self) -> Option<ModuleIdx> {
    match &self.kind {
      ChunkKind::EntryPoint { module, meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry) => {
        Some(*module)
      }
      _ => None,
    }
  }

  pub fn user_defined_entry_module<'module>(
    &self,
    module_table: &'module ModuleTable,
  ) -> Option<&'module NormalModule> {
    self.user_defined_entry_module_idx().and_then(|idx| module_table[idx].as_normal())
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
    self.entry_module_idx().and_then(|idx| module_table[idx].as_normal())
  }

  pub fn is_user_defined_entry(&self) -> bool {
    matches!(&self.kind, ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry))
  }

  pub fn is_async_entry(&self) -> bool {
    matches!(&self.kind, ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::DynamicImported))
  }
}
