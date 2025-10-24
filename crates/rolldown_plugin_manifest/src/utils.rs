use std::ffi::OsString;

use arcstr::ArcStr;
use cow_utils::CowUtils;
use rolldown_common::{Output, OutputAsset, OutputChunk};
use rolldown_utils::pattern_filter::normalize_path;
use serde::Serialize;

use super::ManifestPlugin;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChunk {
  pub file: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub src: Option<String>,
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub is_entry: bool,
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub is_dynamic_entry: bool,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub imports: Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub dynamic_imports: Vec<String>,
  // TODO:
  // #[serde(skip_serializing_if = "Option::is_none")]
  // pub css: Option<Vec<String>>,
  // #[serde(skip_serializing_if = "Option::is_none")]
  // pub assets: Option<Vec<String>>,
}

impl ManifestPlugin {
  pub fn get_chunk_name(&self, chunk: &OutputChunk, is_legacy: bool) -> String {
    match &chunk.facade_module_id {
      Some(module_id) => {
        let mut name = module_id.relative_path(&self.root);
        if is_legacy && !chunk.name.contains("-legacy") {
          let extension = OsString::from(name.extension().unwrap_or_default());
          if let Some(stem) = name.file_stem() {
            let mut file_stem = OsString::with_capacity(stem.len() + 7);
            file_stem.push(stem);
            file_stem.push("-legacy");
            name.set_file_name(file_stem);
          }
          name.set_extension(extension);
        }
        let name = name.to_string_lossy();
        let name = normalize_path(&name);
        name.cow_replace('\0', "").into_owned()
      }
      _ => rolldown_utils::concat_string!(
        "_",
        std::path::Path::new(chunk.filename.as_str()).file_name().unwrap().to_string_lossy()
      ),
    }
  }

  pub fn create_asset(asset: &OutputAsset, src: String, name: Option<String>) -> ManifestChunk {
    let is_entry = name.is_some();
    ManifestChunk {
      name,
      is_entry,
      src: Some(src),
      file: asset.filename.to_string(),
      ..Default::default()
    }
  }

  pub fn create_chunk(
    &self,
    bundle: &Vec<Output>,
    chunk: &OutputChunk,
    src: &str,
    is_legacy: bool,
  ) -> ManifestChunk {
    ManifestChunk {
      file: chunk.filename.to_string(),
      name: Some(chunk.name.to_string()),
      src: chunk.facade_module_id.is_some().then(|| src.to_string()),
      is_entry: chunk.is_entry,
      is_dynamic_entry: chunk.is_dynamic_entry,
      imports: self.get_internal_imports(bundle, &chunk.imports, is_legacy),
      dynamic_imports: self.get_internal_imports(bundle, &chunk.dynamic_imports, is_legacy),
    }
  }

  fn get_internal_imports(
    &self,
    bundle: &Vec<Output>,
    imports: &Vec<ArcStr>,
    is_legacy: bool,
  ) -> Vec<String> {
    let mut filtered_imports = vec![];
    for file in imports {
      for output in bundle {
        if let Output::Chunk(chunk) = output {
          if chunk.filename == *file {
            filtered_imports.push(self.get_chunk_name(chunk, is_legacy));
            break;
          }
        }
      }
    }
    filtered_imports
  }
}

pub fn is_non_js_file(
  file: &str,
  manifest: &std::collections::BTreeMap<String, std::sync::Arc<ManifestChunk>>,
) -> bool {
  manifest.get(file).is_none_or(|m| {
    !(m.file.ends_with(".js") || m.file.ends_with(".cjs") || m.file.ends_with(".mjs"))
  })
}
