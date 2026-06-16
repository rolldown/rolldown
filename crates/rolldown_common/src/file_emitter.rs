use crate::{
  AddEntryModuleMsg, FilenameTemplate, ModuleId, ModuleLoaderMsg, Modules,
  NormalizedBundlerOptions, Output, OutputAsset, OutputChunk, PreserveEntrySignatures, StrOrBytes,
  is_path_fragment,
};
use anyhow::Context;
use arcstr::ArcStr;
use rolldown_error::{BuildDiagnostic, InvalidOptionType};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rolldown_utils::make_unique_name::make_unique_name;
use rolldown_utils::xxhash::{xxhash_base64_url, xxhash_with_base};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use sugar_path::SugarPath;

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
  tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>>>,
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
  /// Maps module IDs to their emitted file reference IDs.
  /// Used by the asset module plugin to associate modules with emitted files
  /// so that the `new URL()` finalizer can look up asset filenames.
  module_to_file_ref: FxDashMap<ArcStr, ArcStr>,
}

impl FileEmitter {
  pub fn new(options: Arc<NormalizedBundlerOptions>) -> Self {
    Self {
      tx: Arc::new(Mutex::new(None)),
      source_hash_to_reference_id: FxDashMap::default(),
      names: FxDashMap::default(),
      files: FxDashMap::default(),
      chunks: FxDashMap::default(),
      prebuilt_chunks: FxDashMap::default(),
      emitted_chunks: FxDashMap::default(),
      base_reference_id: AtomicUsize::new(0),
      options,
      emitted_files: FxDashSet::default(),
      emitted_filenames: FxDashSet::default(),
      module_to_file_ref: FxDashMap::default(),
    }
  }

  pub fn set_emitted_chunk_info(&self, emitted_chunk_info: impl Iterator<Item = EmittedChunkInfo>) {
    for info in emitted_chunk_info {
      self.emitted_chunks.insert_and_forget(info.reference_id, info.filename);
    }
  }

  pub fn emit_chunk(&self, chunk: Arc<EmittedChunk>) -> anyhow::Result<ArcStr> {
    // Must stay synchronous: making this async would force the napi binding back
    // onto `block_on`, pinning the JS thread while `send().await` waits on channel
    // capacity — but the consumer draining the channel itself needs the JS thread
    // to run plugin hooks. That cycle is the emit-chunk deadlock. Keep the channel
    // unbounded (so `send()` never waits) and release the lock before the send.
    let sender = self
      .tx
      .lock()
      .ok()
      .context("Failed to acquire FileEmitter tx lock")?
      .clone()
      .context(
        "The `PluginContext.emitFile` with `type: 'chunk'` only work at `buildStart/resolveId/load/transform/moduleParsed` hooks.",
      )?;
    // Only assign a reference id once we know we have a live sender — keeps
    // `emit_chunk` side-effect-free on the error path.
    let reference_id = self.assign_reference_id(chunk.name.clone());
    sender
      .send(ModuleLoaderMsg::AddEntryModule(Box::new(AddEntryModuleMsg {
        chunk: Arc::clone(&chunk),
        reference_id: reference_id.clone(),
      })))
      .map_err(|e| {
        anyhow::Error::new(e).context(
          "FileEmitter: failed to send AddEntryModule message - module loader shut down during file emission",
        )
      })?;
    self.chunks.insert_and_forget(reference_id.clone(), chunk);
    Ok(reference_id)
  }

  pub fn emit_prebuilt_chunk(&self, chunk: EmittedPrebuiltChunk) -> ArcStr {
    let reference_id = self.assign_reference_id(Some(chunk.file_name.clone()));
    self.prebuilt_chunks.insert_and_forget(reference_id.clone(), Arc::new(chunk));
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
      // Atomically check-and-insert: pre-assign a candidate reference id, then
      // `get_or_insert_with` so concurrent first-emitters agree on a single id.
      // (A losing candidate only wastes one counter tick, which is harmless.)
      let candidate = self.assign_reference_id(None);
      let reference_id =
        self.source_hash_to_reference_id.get_or_insert_with(hash.clone(), || candidate.clone());
      if reference_id != candidate {
        // File already exists, add metadata and return the existing reference_id.
        let name = file.name.clone();
        let original_file_name = file.original_file_name.clone();
        self.files.pin().update(reference_id.clone(), |output| {
          let mut output = output.clone();
          if let Some(name) = name.clone() {
            output.names.push(name);
          }
          if let Some(original_file_name) = original_file_name.clone() {
            output.original_file_names.push(original_file_name);
          }
          output
        });
        return Ok(reference_id);
      }
      reference_id
    } else {
      // File has explicit fileName, no deduplication needed
      self.assign_reference_id(file.file_name.clone())
    };

    // Generate filename and insert into files map
    self.generate_file_name(&mut file, &hash, asset_filename_template, sanitized_file_name)?;
    self.files.insert_and_forget(
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
    if let Some(filename) = self.files.with(reference_id, |file| file.filename.clone()) {
      return Ok(filename);
    }
    if let Some(chunk) = self.chunks.get(reference_id) {
      if let Some(filename) = chunk.file_name.as_ref() {
        return Ok(filename.clone());
      }
      if let Some(filename) = self.emitted_chunks.get(reference_id) {
        return Ok(filename);
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
    let mut additional_assets = Vec::new();
    let files = self.files.pin();
    for (key, value) in &files {
      if !self.emitted_files.insert(key.clone()) {
        continue;
      }

      // Follow rollup using lowercase filename to check conflicts
      let lowercase_filename = value.filename.as_str().to_lowercase().into();
      if !self.emitted_filenames.insert(lowercase_filename) {
        warnings
          .push(BuildDiagnostic::filename_conflict(value.filename.clone()).with_severity_warning());
      }

      let mut names = value.names.clone();
      sort_names(&mut names);

      let mut original_file_names = value.original_file_names.clone();
      original_file_names.sort_unstable();
      additional_assets.push(Output::Asset(Arc::new(OutputAsset {
        filename: value.filename.clone(),
        names,
        original_file_names,
        source: value.source.clone(),
      })));
    }
    drop(files);
    // Sort to ensure deterministic output order regardless of map iteration order
    additional_assets.sort_unstable_by(|a, b| a.filename().cmp(b.filename()));
    bundle.extend(additional_assets);

    // Add prebuilt chunks to the bundle
    let prebuilt_chunks = self.prebuilt_chunks.pin();
    for (key, value) in &prebuilt_chunks {
      if !self.emitted_files.insert(key.clone()) {
        continue;
      }

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
    }
  }

  pub fn set_context_load_modules_tx(
    &self,
    tx: Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>,
  ) -> anyhow::Result<()> {
    *self.tx.lock().ok().context("Failed to acquire FileEmitter tx lock")? = tx;
    Ok(())
  }

  /// Associate a module ID with an emitted file reference ID.
  /// This allows the `new URL()` finalizer to look up asset filenames by module ID.
  pub fn associate_module_with_file_ref(&self, module_id: &str, reference_id: &str) {
    self.module_to_file_ref.insert_and_forget(ArcStr::from(module_id), ArcStr::from(reference_id));
  }

  /// Get the emitted file reference ID for a given module ID.
  pub fn file_ref_for_module(&self, module_id: &str) -> Option<ArcStr> {
    self.module_to_file_ref.get(module_id)
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
    self.emitted_filenames.clear();
    self.module_to_file_ref.clear();
  }
}

fn sort_names(names: &mut [String]) {
  names.sort_unstable_by(|a, b| {
    let len_ord = a.len().cmp(&b.len());
    if len_ord == std::cmp::Ordering::Equal { a.cmp(b) } else { len_ord }
  });
}

pub type SharedFileEmitter = Arc<FileEmitter>;
