use crate::{
  ExtraEntryModuleData, FileNameRenderOptions, ModuleLoaderMsg, NormalizedBundlerOptions, Output,
  OutputAsset, ResolvedId, StrOrBytes,
};
use anyhow::Context;
use arcstr::ArcStr;
use dashmap::{DashMap, DashSet};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use rolldown_utils::extract_hash_pattern::extract_hash_pattern;
use rolldown_utils::sanitize_file_name::sanitize_file_name;
use rolldown_utils::xxhash::xxhash_base64_url;
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

#[derive(Debug)]
pub struct EmittedChunk {
  pub name: Option<ArcStr>,
  pub file_name: Option<ArcStr>,
  pub id: String,
  // pub implicitly_loaded_after_one_of: Option<Vec<String>>,
  pub importer: Option<String>,
}

#[derive(Debug)]
pub struct FileEmitter {
  tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
  source_hash_to_reference_id: FxDashMap<ArcStr, ArcStr>,
  names: FxDashMap<ArcStr, u32>,
  files: FxDashMap<ArcStr, OutputAsset>,
  chunks: FxDashMap<ArcStr, EmittedChunk>,
  base_reference_id: AtomicUsize,
  options: Arc<NormalizedBundlerOptions>,
  /// Mark the files that have been emitted to bundle.
  emitted_files: FxDashSet<ArcStr>,
}

impl FileEmitter {
  pub fn new(options: Arc<NormalizedBundlerOptions>) -> Self {
    Self {
      tx: Arc::new(Mutex::new(None)),
      source_hash_to_reference_id: DashMap::default(),
      names: DashMap::default(),
      files: DashMap::default(),
      chunks: DashMap::default(),
      base_reference_id: AtomicUsize::new(0),
      options,
      emitted_files: DashSet::default(),
    }
  }

  pub async fn emit_chunk(
    &self,
    chunk: EmittedChunk,
    resolved_id: ResolvedId,
  ) -> anyhow::Result<ArcStr> {
    let reference_id = self.assign_reference_id(Some(chunk.id.as_str().into()));
    self
    .tx
    .lock()
    .await
    .as_ref()
    .context(
      "The `PluginContext.emitFile` with `type: 'chunk'` only work at `buildStart/resolveId/load/transform/moduleParsed` hooks.",
    )?
    .send(ModuleLoaderMsg::AddEntryModule(ExtraEntryModuleData { file_name: chunk.file_name.clone(), name: chunk.name.clone(), resolved_id }))
    .await?;
    self.chunks.insert(reference_id.clone(), chunk);
    Ok(reference_id)
  }

  pub fn emit_file(&self, mut file: EmittedAsset) -> ArcStr {
    let hash: ArcStr = xxhash_base64_url(file.source.as_bytes()).into();
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

    self.generate_file_name(&mut file, &hash);
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

  pub fn try_get_file_name(&self, reference_id: &str) -> Result<ArcStr, String> {
    let file = self
      .files
      .get(reference_id)
      .ok_or(format!("Unable to get file name for unknown file: {reference_id}"))?;
    Ok(file.filename.clone())
  }

  // TODO support EmittedChunk
  pub fn get_file_name(&self, reference_id: &str) -> ArcStr {
    self
      .try_get_file_name(reference_id)
      .unwrap_or_else(|_| panic!("{reference_id} should have file name"))
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

  pub fn generate_file_name(&self, file: &mut EmittedAsset, hash: &ArcStr) {
    if file.file_name.is_none() {
      let path = file.name.as_deref().map(Path::new);
      let extension = path.and_then(|x| x.extension().and_then(OsStr::to_str));
      let name = path
        .and_then(|x| x.file_stem().and_then(OsStr::to_str))
        .map(|x| sanitize_file_name(x.into()));
      let extract_hash_pattern = extract_hash_pattern(self.options.asset_filenames.template());
      let mut file_name: ArcStr = self
        .options
        .asset_filenames
        .render(&FileNameRenderOptions {
          name: name.as_deref(),
          hash: extract_hash_pattern
            .map(|p| &hash.as_str()[..p.len.map_or(8, |hash_len| hash_len.max(6))]),
          ext: extension,
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
    self.files.clear();
    self.names.clear();
    self.source_hash_to_reference_id.clear();
    self.base_reference_id.store(0, Ordering::Relaxed);
    self.emitted_files.clear();
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
