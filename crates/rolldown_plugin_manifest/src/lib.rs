use rolldown_common::{EmittedAsset, ModuleId, Output, OutputAsset, OutputChunk};
use rolldown_plugin::{HookNoopReturn, Plugin, PluginContext};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;
use std::{borrow::Cow, collections::BTreeMap, path::Path, rc::Rc, sync::LazyLock};

#[derive(Debug)]
pub struct ManifestPlugin {
  pub config: ManifestPluginConfig,
}

#[derive(Debug)]
pub struct GeneratedAssetMeta {
  pub original_name: String,
  pub is_entry: bool,
}

// TODO: Link this with assets plugin
static GENERATED_ASSETS: LazyLock<FxHashMap<String, GeneratedAssetMeta>> =
  LazyLock::new(FxHashMap::default);

#[derive(Debug, Default)]
pub struct ManifestPluginConfig {
  pub root: String,
  pub out_path: String,
}

impl Plugin for ManifestPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:manifest-plugin")
  }

  #[allow(clippy::case_sensitive_file_extension_comparisons)]
  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    bundle: &mut Vec<Output>,
    _is_write: bool,
  ) -> HookNoopReturn {
    // Use BTreeMap to make the result sorted
    let mut manifest = BTreeMap::default();
    let mut file_name_to_asset = FxHashMap::default();
    let mut file_name_to_asset_meta = FxHashMap::default();
    let assets: &FxHashMap<String, GeneratedAssetMeta> = &GENERATED_ASSETS;
    let mut skip_assets = FxHashSet::default();
    for (reference_id, asset) in assets {
      if let Ok(file_name) = ctx.try_get_file_name(reference_id.as_str()) {
        file_name_to_asset_meta.insert(file_name, asset);
      } else {
        // The asset was generated as part of a different output option.
        // It was already handled during the previous run of this plugin.
        skip_assets.insert(reference_id);
      }
    }

    for file in bundle.iter() {
      match file {
        Output::Chunk(chunk) => {
          let name = self.get_chunk_name(chunk);
          let chunk_manifest = Rc::new(self.create_chunk(bundle, chunk, name.clone()));
          manifest.insert(name.clone(), chunk_manifest);
        }
        Output::Asset(asset) => {
          if let Some(name) = &asset.name {
            let asset_meta = file_name_to_asset_meta.remove(&asset.filename);
            let src = asset_meta.map_or(name, |m| &m.original_name);
            let asset_manifest = Rc::new(Self::create_asset(
              asset,
              src.clone(),
              asset_meta.map_or(false, |m| m.is_entry),
            ));

            // If JS chunk and asset chunk are both generated from the same source file,
            // prioritize JS chunk as it contains more information
            if let Some(m) = manifest.get(src) {
              let file = &m.file;

              if file.ends_with(".js") || file.ends_with(".cjs") || file.ends_with(".mjs") {
                continue;
              }
            }

            manifest.insert(src.clone(), Rc::<ManifestChunk>::clone(&asset_manifest));
            file_name_to_asset.insert(asset.filename.clone(), asset_manifest);
          }
        }
      }
    }

    // Add deduplicated assets to the manifest
    for (reference_id, asset) in assets {
      if skip_assets.contains(reference_id) {
        continue;
      }
      let original_name = &asset.original_name;
      if !manifest.contains_key(original_name) {
        let filename = ctx.get_file_name(reference_id.as_str());
        let asset = file_name_to_asset.remove(&filename);
        if let Some(asset) = asset {
          manifest.insert(original_name.clone(), asset);
        }
      }
    }

    // TODO: uncomment these when multiple outputs are supported
    // output_count += 1;
    // let output = config.build.rollupOptions?.output
    // let outputLength = Array.isArray(output) ? output.length : 1
    // if output_count >= outputLength {
    ctx.emit_file(EmittedAsset {
      file_name: Some(self.config.out_path.clone()),
      name: None,
      original_file_name: None,
      source: (serde_json::to_string_pretty(&manifest).unwrap()).into(),
    });
    // }

    Ok(())
  }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
  !b
}

#[derive(Debug, Default, Serialize)]
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
  fn get_chunk_name(&self, chunk: &OutputChunk) -> String {
    get_chunk_original_file_name(chunk, &self.config.root)
  }
  fn get_internal_imports(&self, bundle: &Vec<Output>, imports: &Vec<ModuleId>) -> Vec<String> {
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
  fn create_chunk(&self, bundle: &Vec<Output>, chunk: &OutputChunk, src: String) -> ManifestChunk {
    ManifestChunk {
      file: chunk.filename.to_string(),
      name: Some(chunk.name.to_string()),
      src: if chunk.facade_module_id.is_some() { Some(src) } else { None },
      is_entry: chunk.is_entry,
      is_dynamic_entry: chunk.is_dynamic_entry,
      imports: self.get_internal_imports(bundle, &chunk.imports),
      dynamic_imports: self.get_internal_imports(bundle, &chunk.dynamic_imports),
    }
  }
  fn create_asset(asset: &OutputAsset, src: String, is_entry: bool) -> ManifestChunk {
    ManifestChunk {
      file: asset.filename.to_string(),
      src: Some(src),
      is_entry,
      ..Default::default()
    }
  }
}

fn get_chunk_original_file_name(chunk: &OutputChunk, root: &str) -> String {
  if let Some(facade_module_id) = &chunk.facade_module_id {
    let name = facade_module_id.relative_path(root);
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
  } else {
    format!("_{}", Path::new(chunk.filename.as_str()).file_name().unwrap().to_string_lossy())
  }
}
