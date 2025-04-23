use arcstr::ArcStr;
use rolldown_common::{Output, OutputAsset, OutputChunk};
use serde::Serialize;

use super::ManifestPlugin;

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
  !b
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestChunk {
  pub file: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub src: Option<String>,
  #[serde(skip_serializing_if = "is_false")]
  pub is_entry: bool,
  #[serde(skip_serializing_if = "is_false")]
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
  pub fn get_chunk_name(&self, chunk: &OutputChunk) -> String {
    match &chunk.facade_module_id {
      Some(facade_module_id) => {
        let name = facade_module_id.relative_path(&self.config.root);
        let name_str = name.to_string_lossy().to_string();
        // TODO: Support System format
        // if format == 'system' && !chunk.name.as_str().contains("-legacy") {
        //   name_str = if let Some(ext) = name.extension() {
        //     let end = name_str.len() - ext.len() - 1;
        //     format!("{}-legacy.{}", &name_str[0..end], ext.to_string_lossy())
        //   } else {
        //     format!("{name_str}-legacy")
        //   }
        // }
        name_str.replace('\0', "")
      }
      _ => {
        format!(
          "_{}",
          std::path::Path::new(chunk.filename.as_str()).file_name().unwrap().to_string_lossy()
        )
      }
    }
  }
  fn get_internal_imports(&self, bundle: &Vec<Output>, imports: &Vec<ArcStr>) -> Vec<String> {
    let mut filtered_imports = vec![];
    for file in imports {
      for chunk in bundle {
        if let Output::Chunk(output_chunk) = chunk {
          if output_chunk.filename == *file {
            filtered_imports.push(self.get_chunk_name(output_chunk));
            break;
          }
        }
      }
    }
    filtered_imports
  }
  pub fn create_chunk(
    &self,
    bundle: &Vec<Output>,
    chunk: &OutputChunk,
    src: String,
  ) -> ManifestChunk {
    ManifestChunk {
      file: chunk.filename.to_string(),
      name: Some(chunk.name.to_string()),
      src: chunk.facade_module_id.is_some().then_some(src),
      is_entry: chunk.is_entry,
      is_dynamic_entry: chunk.is_dynamic_entry,
      imports: self.get_internal_imports(bundle, &chunk.imports),
      dynamic_imports: self.get_internal_imports(bundle, &chunk.dynamic_imports),
    }
  }
  pub fn create_asset(asset: &OutputAsset, src: String, is_entry: bool) -> ManifestChunk {
    ManifestChunk {
      file: asset.filename.to_string(),
      src: Some(src),
      is_entry,
      ..Default::default()
    }
  }
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn is_non_js_file(
  file: &str,
  manifest: &std::collections::BTreeMap<String, std::sync::Arc<ManifestChunk>>,
) -> bool {
  manifest.get(file).is_none_or(|m| {
    !(m.file.ends_with(".js") || m.file.ends_with(".cjs") || m.file.ends_with(".mjs"))
  })
}
