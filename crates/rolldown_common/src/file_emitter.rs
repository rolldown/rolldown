use crate::{FileNameRenderOptions, NormalizedBundlerOptions, Output, OutputAsset};
use dashmap::{DashMap, DashSet};
use rolldown_utils::xxhash::xxhash_base64_url;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct EmittedAsset {
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub source: String,
}

#[derive(Debug)]
pub struct FileEmitter {
  files: DashMap<String, EmittedAsset>,
  base_reference_id: AtomicUsize,
  options: Arc<NormalizedBundlerOptions>,
  /// Mark the files that have been emitted to bundle.
  emitted_files: DashSet<String>,
}

impl FileEmitter {
  pub fn new(options: Arc<NormalizedBundlerOptions>) -> Self {
    Self {
      files: DashMap::default(),
      base_reference_id: AtomicUsize::new(0),
      options,
      emitted_files: DashSet::default(),
    }
  }

  pub fn emit_file(&self, mut file: EmittedAsset) -> String {
    let reference_id = self.assign_reference_id(file.file_name.clone());
    self.generate_file_name(&mut file);
    self.files.insert(reference_id.clone(), file);
    reference_id
  }

  pub fn get_file_name(&self, reference_id: &str) -> String {
    self.files.get(reference_id).map_or_else(
      || panic!("Unable to get file name for unknown file: {reference_id}"),
      |file| file.file_name.clone().expect(" should have file name"),
    )
  }

  pub fn assign_reference_id(&self, filename: Option<String>) -> String {
    xxhash_base64_url(
      filename
        .unwrap_or_else(|| self.base_reference_id.fetch_add(0, Ordering::Relaxed).to_string())
        .as_bytes(),
    )
  }

  pub fn generate_file_name(&self, file: &mut EmittedAsset) {
    if file.file_name.is_none() {
      let path = file.name.as_deref().map(Path::new);
      let extension = path.and_then(|x| x.extension().and_then(OsStr::to_str));
      let file_name = self.options.asset_filenames.render(&FileNameRenderOptions {
        name: path.and_then(|x| x.file_stem().and_then(OsStr::to_str)),
        hash: Some(xxhash_base64_url(file.source.as_bytes()).as_str()),
        ext: extension,
      });
      file.file_name = Some(file_name);
    }
  }

  pub fn add_additional_files(&self, bundle: &mut Vec<Output>) {
    for file in &self.files {
      let (key, value) = file.pair();
      if self.emitted_files.contains(key) {
        continue;
      }
      self.emitted_files.insert(key.clone());
      bundle.push(Output::Asset(Box::new(OutputAsset {
        filename: value.file_name.clone().expect("should have file name"),
        source: value.source.clone(),
      })));
    }
  }
}

pub type SharedFileEmitter = Arc<FileEmitter>;
