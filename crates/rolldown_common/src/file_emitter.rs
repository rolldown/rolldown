use crate::{
  AddEntryModuleMsg, FileNameRenderOptions, FilenameTemplate, ModuleLoaderMsg,
  NormalizedBundlerOptions, Output, OutputAsset, StrOrBytes,
};
use anyhow::Context;
use arcstr::ArcStr;
use dashmap::{DashMap, DashSet};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rolldown_utils::extract_hash_pattern::extract_hash_pattern;
use rolldown_utils::xxhash::{xxhash_base64_url, xxhash_with_base};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
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
}

#[derive(Debug, Default)]
pub struct EmittedChunk {
  pub name: Option<ArcStr>,
  pub file_name: Option<ArcStr>,
  pub id: String,
  // pub implicitly_loaded_after_one_of: Option<Vec<String>>,
  pub importer: Option<String>,
}

pub struct EmittedChunkInfo {
  pub reference_id: ArcStr,
  pub filename: ArcStr,
}

#[derive(Debug)]
pub struct FileEmitter {
  tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
  source_hash_to_reference_id: FxDashMap<ArcStr, ArcStr>,
  names: FxDashMap<ArcStr, u32>,
  files: FxDashMap<ArcStr, OutputAsset>,
  chunks: FxDashMap<ArcStr, Arc<EmittedChunk>>,
  base_reference_id: AtomicUsize,
  #[allow(dead_code)]
  options: Arc<NormalizedBundlerOptions>,
  /// Mark the files that have been emitted to bundle.
  emitted_files: FxDashSet<ArcStr>,
  emitted_chunks: FxDashMap<ArcStr, ArcStr>,
}

impl FileEmitter {
  pub fn new(options: Arc<NormalizedBundlerOptions>) -> Self {
    Self {
      tx: Arc::new(Mutex::new(None)),
      source_hash_to_reference_id: DashMap::default(),
      names: DashMap::default(),
      files: DashMap::default(),
      chunks: DashMap::default(),
      emitted_chunks: DashMap::default(),
      base_reference_id: AtomicUsize::new(0),
      options,
      emitted_files: DashSet::default(),
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
    .send(ModuleLoaderMsg::AddEntryModule(AddEntryModuleMsg { chunk: Arc::clone(&chunk), reference_id: reference_id.clone() }))
    .await?;
    self.chunks.insert(reference_id.clone(), chunk);
    Ok(reference_id)
  }

  pub fn emit_file(
    &self,
    mut file: EmittedAsset,
    asset_filename_template: Option<FilenameTemplate>,
    sanitized_file_name: Option<ArcStr>,
  ) -> ArcStr {
    let hash: ArcStr =
      xxhash_with_base(file.source.as_bytes(), self.options.hash_characters.base()).into();
    // Deduplicate assets if an explicit fileName is not provided
    if file.file_name.is_none() {
      if let Some(reference_id) = self.source_hash_to_reference_id.get(&hash) {
        self.files.entry(reference_id.clone()).and_modify(|entry| {
          if let Some(name) = file.name {
            entry.names.push(name);
          }
          if let Some(original_file_name) = file.original_file_name {
            entry.original_file_names.push(original_file_name);
          }
        });
        return reference_id.value().clone();
      }
    }

    let reference_id = self.assign_reference_id(file.file_name.clone());
    if file.file_name.is_none() {
      self.source_hash_to_reference_id.insert(hash.clone(), reference_id.clone());
    }

    self.generate_file_name(&mut file, &hash, asset_filename_template, sanitized_file_name);
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
    reference_id
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
      return Err(
        anyhow::anyhow!("Unable to get file name for emitted chunk: {reference_id}.You can only get file names once chunks have been generated after the 'renderStart' hook."),
      );
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
    asset_filename_template: Option<FilenameTemplate>,
    sanitized_file_name: Option<ArcStr>,
  ) {
    if file.file_name.is_none() {
      let sanitized_file_name = sanitized_file_name.expect("should has sanitized file name");
      let path = Path::new(sanitized_file_name.as_str());
      let extension = path.extension().and_then(OsStr::to_str);
      let name = path.file_stem().and_then(OsStr::to_str);
      let asset_filename_template =
        asset_filename_template.expect("should has filename template without filename");
      let extract_hash_pattern = extract_hash_pattern(asset_filename_template.template());
      let mut file_name: ArcStr = asset_filename_template
        .render(&FileNameRenderOptions {
          name,
          hash: extract_hash_pattern
            .map(|p| &hash.as_str()[..p.len.map_or(8, |hash_len| hash_len.max(6))]),
          ext: Some(extension.unwrap_or_default()),
        })
        .into();
      // deconflict file name
      if let Some(count) = self.names.get_mut(file_name.as_str()).as_deref_mut() {
        *count += 1;
        let extension = extension.map(|e| format!(".{e}")).unwrap_or_default();
        file_name = format!(
          "{}{count}{extension}",
          &file_name.to_string()[..file_name.len() - extension.len()],
        )
        .into();
      } else {
        self.names.insert(file_name.clone(), 1);
      }

      file.file_name = Some(file_name);
    }
  }

  pub fn add_additional_files(&self, bundle: &mut Vec<Output>) {
    self.files.iter_mut().for_each(|mut file| {
      let (key, value) = file.pair_mut();
      if self.emitted_files.contains(key) {
        return;
      }
      self.emitted_files.insert(key.clone());

      let mut names = std::mem::take(&mut value.names);
      sort_names(&mut names);

      let mut original_file_names = std::mem::take(&mut value.original_file_names);
      original_file_names.sort_unstable();
      bundle.push(Output::Asset(Box::new(OutputAsset {
        filename: value.filename.clone(),
        names,
        original_file_names,
        source: std::mem::take(&mut value.source),
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
    if len_ord == std::cmp::Ordering::Equal {
      a.cmp(b)
    } else {
      len_ord
    }
  });
}

pub type SharedFileEmitter = Arc<FileEmitter>;
