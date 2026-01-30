use crate::{
  AddEntryModuleMsg, FilenameTemplate, ModuleId, ModuleLoaderMsg, Modules,
  NormalizedBundlerOptions, Output, OutputAsset, OutputChunk, PreserveEntrySignatures, StrOrBytes,
  is_path_fragment,
};
use anyhow::Context;
use arcstr::ArcStr;
use dashmap::{DashMap, DashSet, Entry};
use rolldown_error::{BuildDiagnostic, InvalidOptionType};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rolldown_utils::make_unique_name::make_unique_name;
use rolldown_utils::xxhash::{xxhash_base64_url, xxhash_with_base};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use sugar_path::SugarPath;
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct EmittedAsset {
  pub name: Option<String>,
  pub original_file_name: Option<String>,
  pub file_name: Option<ArcStr>,
  pub source: StrOrBytes,
}

impl EmittedAsset {
  pub fn name_for_sanitize(&self) -> &str {
    self.name.as_deref().unwrap_or("asset")
  }

  /// Returns true if the emitted asset has a valid name (not an absolute or relative path).
  /// Similar to Rollup's `hasValidName` function.
  pub fn has_valid_name(&self) -> bool {
    let validated_name = self.file_name.as_deref().or(self.name.as_deref());
    validated_name.is_none_or(|name| !is_path_fragment(name))
  }

  /// Returns the validated name (fileName or name) if present.
  pub fn validated_name(&self) -> Option<&str> {
    self.file_name.as_deref().or(self.name.as_deref())
  }
}

#[derive(Debug, Default)]
pub struct EmittedChunk {
  pub name: Option<ArcStr>,
  pub file_name: Option<ArcStr>,
  pub id: String,
  // pub implicitly_loaded_after_one_of: Option<Vec<String>>,
  pub importer: Option<String>,
  pub preserve_entry_signatures: Option<PreserveEntrySignatures>,
}

pub struct EmittedChunkInfo {
  pub reference_id: ArcStr,
  pub filename: ArcStr,
}

#[derive(Debug, Clone)]
pub struct EmittedPrebuiltChunk {
  pub file_name: ArcStr,
  pub name: Option<ArcStr>,
  pub code: String,
  pub exports: Vec<String>,
  pub map: Option<rolldown_sourcemap::SourceMap>,
  pub sourcemap_filename: Option<String>,
  pub facade_module_id: Option<ArcStr>,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
}

#[derive(Debug)]
pub struct FileEmitter {
  tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
  source_hash_to_reference_id: FxDashMap<ArcStr, ArcStr>,
  names: FxDashMap<ArcStr, u32>,
  files: FxDashMap<ArcStr, OutputAsset>,
  chunks: FxDashMap<ArcStr, Arc<EmittedChunk>>,
  prebuilt_chunks: FxDashMap<ArcStr, Arc<EmittedPrebuiltChunk>>,
  base_reference_id: AtomicUsize,
  options: Arc<NormalizedBundlerOptions>,
  /// Mark the files that have been emitted to bundle.
  emitted_files: FxDashSet<ArcStr>,
  emitted_chunks: FxDashMap<ArcStr, ArcStr>,
  emitted_filenames: FxDashSet<ArcStr>,
}

impl FileEmitter {
  pub fn new(options: Arc<NormalizedBundlerOptions>) -> Self {
    Self {
      tx: Arc::new(Mutex::new(None)),
      source_hash_to_reference_id: DashMap::default(),
      names: DashMap::default(),
      files: DashMap::default(),
      chunks: DashMap::default(),
      prebuilt_chunks: DashMap::default(),
      emitted_chunks: DashMap::default(),
      base_reference_id: AtomicUsize::new(0),
      options,
      emitted_files: DashSet::default(),
      emitted_filenames: FxDashSet::default(),
    }
  }

  pub fn set_emitted_chunk_info(&self, emitted_chunk_info: impl Iterator<Item = EmittedChunkInfo>) {
    for info in emitted_chunk_info {
      self.emitted_chunks.insert(info.reference_id, info.filename);
    }
  }

  pub async fn emit_chunk(&self, chunk: Arc<EmittedChunk>) -> anyhow::Result<ArcStr> {
    let reference_id = self.assign_reference_id(chunk.name.clone());
    self
    .tx
    .lock()
    .await
    .as_ref()
    .context(
      "The `PluginContext.emitFile` with `type: 'chunk'` only work at `buildStart/resolveId/load/transform/moduleParsed` hooks.",
    )?
    .send(ModuleLoaderMsg::AddEntryModule(Box::new(AddEntryModuleMsg { chunk: Arc::clone(&chunk), reference_id: reference_id.clone() })))
    .await
    .context("FileEmitter: failed to send AddEntryModule message - module loader shut down during file emission")?;
    self.chunks.insert(reference_id.clone(), chunk);
    Ok(reference_id)
  }

  pub fn emit_prebuilt_chunk(&self, chunk: EmittedPrebuiltChunk) -> ArcStr {
    let reference_id = self.assign_reference_id(Some(chunk.file_name.clone()));
    self.prebuilt_chunks.insert(reference_id.clone(), Arc::new(chunk));
    reference_id
  }

  pub fn emit_file(
    &self,
    mut file: EmittedAsset,
    asset_filename_template: Option<FilenameTemplate>,
    sanitized_file_name: Option<ArcStr>,
  ) -> anyhow::Result<ArcStr> {
    if !file.has_valid_name() {
      return Err(
        BuildDiagnostic::invalid_option(InvalidOptionType::InvalidEmittedFileName(
          file.validated_name().unwrap_or_default().to_string(),
        ))
        .into(),
      );
    }

    let hash: ArcStr =
      xxhash_with_base(file.source.as_bytes(), self.options.hash_characters.base()).into();

    // Deduplicate assets if an explicit fileName is not provided
    let reference_id = if file.file_name.is_none() {
      // Use entry API to atomically check and insert
      match self.source_hash_to_reference_id.entry(hash.clone()) {
        Entry::Occupied(entry) => {
          // File already exists, add metadata and return existing reference_id
          let reference_id = entry.get().clone();
          self.files.entry(reference_id.clone()).and_modify(|output| {
            if let Some(name) = file.name {
              output.names.push(name);
            }
            if let Some(original_file_name) = file.original_file_name {
              output.original_file_names.push(original_file_name);
            }
          });
          return Ok(reference_id);
        }
        Entry::Vacant(entry) => {
          // First time seeing this file, generate reference_id and continue
          let reference_id = self.assign_reference_id(None);
          entry.insert(reference_id.clone());
          reference_id
        }
      }
    } else {
      // File has explicit fileName, no deduplication needed
      self.assign_reference_id(file.file_name.clone())
    };

    // Generate filename and insert into files map
    self.generate_file_name(&mut file, &hash, asset_filename_template, sanitized_file_name)?;
    self.files.insert(
      reference_id.clone(),
      OutputAsset {
        filename: file.file_name.unwrap(),
        source: std::mem::take(&mut file.source),
        names: std::mem::take(&mut file.name).map_or(vec![], |name| vec![name]),
        original_file_names: std::mem::take(&mut file.original_file_name)
          .map_or(vec![], |original_file_name| vec![original_file_name]),
      },
    );
    Ok(reference_id)
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    if let Some(file) = self.files.get(reference_id) {
      return Ok(file.filename.clone());
    }
    if let Some(chunk) = self.chunks.get(reference_id) {
      if let Some(filename) = chunk.file_name.as_ref() {
        return Ok(filename.clone());
      }
      if let Some(filename) = self.emitted_chunks.get(reference_id) {
        return Ok(filename.clone());
      }
      return Err(anyhow::anyhow!(
        "Unable to get file name for emitted chunk: {reference_id}.You can only get file names once chunks have been generated after the 'renderStart' hook."
      ));
    }
    if let Some(prebuilt_chunk) = self.prebuilt_chunks.get(reference_id) {
      return Ok(prebuilt_chunk.file_name.clone());
    }
    Err(anyhow::anyhow!("Unable to get file name for unknown file: {reference_id}"))
  }

  pub fn assign_reference_id(&self, filename: Option<ArcStr>) -> ArcStr {
    xxhash_base64_url(
      filename
        .unwrap_or_else(|| {
          self.base_reference_id.fetch_add(1, Ordering::Relaxed).to_string().into()
        })
        .as_bytes(),
    )
    // The reference id can be used for import.meta.ROLLUP_FILE_URL_referenceId and therefore needs to be a valid identifier.
    .replace('-', "$")
    .into()
  }

  pub fn generate_file_name(
    &self,
    file: &mut EmittedAsset,
    hash: &ArcStr,
    filename_template: Option<FilenameTemplate>,
    sanitized_file_name: Option<ArcStr>,
  ) -> anyhow::Result<()> {
    if file.file_name.is_none() {
      let sanitized_file_name = sanitized_file_name.expect("should has sanitized file name");
      let path = Path::new(sanitized_file_name.as_str());
      // Extract extension from the filename only
      let extension = path.extension().and_then(OsStr::to_str);
      // Extract name including directory path, but without extension
      // e.g., "foo/bar.txt" -> "foo/bar", "bar.txt" -> "bar"
      // Security: normalize path and filter out dangerous components
      let name = path.file_stem().and_then(OsStr::to_str).map(|stem| {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
          // Normalize to resolve ".." and "." where possible, then convert to forward slashes
          parent.join(stem).normalize().to_slash_lossy().into_owned()
        } else {
          stem.to_string()
        }
      });
      let filename_template =
        filename_template.expect("should has filename template without filename");

      let mut filename = filename_template
        .render(
          name.as_deref(),
          None,
          Some(extension.unwrap_or_default()),
          Some(|len: Option<usize>| Ok(&hash[..len.map_or(8, |len| len.clamp(1, 21))])),
        )?
        .into();

      // deconflict file name
      // TODO(underfin): could be using bundle files key as `make_unique_name`
      filename = make_unique_name(&filename, &self.names);

      file.file_name = Some(filename);
    }
    Ok(())
  }

  pub fn add_additional_files(
    &self,
    bundle: &mut Vec<Output>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) {
    self.files.iter_mut().for_each(|mut file| {
      let (key, value) = file.pair_mut();
      if self.emitted_files.contains(key) {
        return;
      }
      self.emitted_files.insert(key.clone());

      // Follow rollup using lowercase filename to check conflicts
      let lowercase_filename = value.filename.as_str().to_lowercase().into();
      if !self.emitted_filenames.insert(lowercase_filename) {
        warnings
          .push(BuildDiagnostic::filename_conflict(value.filename.clone()).with_severity_warning());
      }

      let mut names = std::mem::take(&mut value.names);
      sort_names(&mut names);

      let mut original_file_names = std::mem::take(&mut value.original_file_names);
      original_file_names.sort_unstable();
      bundle.push(Output::Asset(Arc::new(OutputAsset {
        filename: value.filename.clone(),
        names,
        original_file_names,
        source: std::mem::take(&mut value.source),
      })));
    });

    // Add prebuilt chunks to the bundle
    self.prebuilt_chunks.iter().for_each(|prebuilt_chunk| {
      let (key, value) = prebuilt_chunk.pair();
      if self.emitted_files.contains(key) {
        return;
      }
      self.emitted_files.insert(key.clone());

      // Check for filename conflicts
      let lowercase_filename: ArcStr = value.file_name.as_str().to_lowercase().into();
      if !self.emitted_filenames.insert(lowercase_filename) {
        warnings.push(
          BuildDiagnostic::filename_conflict(value.file_name.clone()).with_severity_warning(),
        );
      }

      bundle.push(Output::Chunk(Arc::new(OutputChunk {
        name: value.name.clone().unwrap_or_else(|| value.file_name.clone()),
        is_entry: value.is_entry,
        is_dynamic_entry: value.is_dynamic_entry,
        facade_module_id: value.facade_module_id.clone().map(ModuleId::from),
        module_ids: vec![],
        exports: value.exports.iter().map(|s| s.as_str().into()).collect(),
        filename: value.file_name.clone(),
        modules: Modules { keys: vec![], values: vec![] },
        imports: vec![],
        dynamic_imports: vec![],
        code: value.code.clone(),
        map: value.map.clone(),
        sourcemap_filename: value.sourcemap_filename.clone(),
        preliminary_filename: value.file_name.to_string(),
      })));
    });
  }

  pub async fn set_context_load_modules_tx(
    &self,
    tx: Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>,
  ) {
    let mut tx_guard = self.tx.lock().await;
    *tx_guard = tx;
  }

  pub fn clear(&self) {
    self.chunks.clear();
    self.files.clear();
    self.prebuilt_chunks.clear();
    self.names.clear();
    self.source_hash_to_reference_id.clear();
    self.base_reference_id.store(0, Ordering::Relaxed);
    self.emitted_files.clear();
    self.emitted_chunks.clear();
  }
}

fn sort_names(names: &mut [String]) {
  names.sort_unstable_by(|a, b| {
    let len_ord = a.len().cmp(&b.len());
    if len_ord == std::cmp::Ordering::Equal { a.cmp(b) } else { len_ord }
  });
}

pub type SharedFileEmitter = Arc<FileEmitter>;
