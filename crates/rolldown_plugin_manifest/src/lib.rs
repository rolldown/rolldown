use std::{borrow::Cow, collections::BTreeMap, path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, Output, OutputAsset, OutputChunk};
use rolldown_plugin::{HookNoopReturn, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::FxHashSet;
use serde::Serialize;

#[derive(Debug)]
pub struct ManifestPlugin {
  pub config: ManifestPluginConfig,
  pub entry_css_asset_file_names: FxHashSet<String>,
}

#[derive(Debug, Default)]
pub struct ManifestPluginConfig {
  pub root: String,
  pub out_path: String,
}

impl Plugin for ManifestPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:manifest")
  }

  #[allow(clippy::case_sensitive_file_extension_comparisons)]
  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    // Use BTreeMap to make the result sorted
    let mut manifest = BTreeMap::default();

    let entry_css_reference_ids: &FxHashSet<String> = &self.entry_css_asset_file_names;
    let mut entry_css_asset_file_names = FxHashSet::with_capacity(entry_css_reference_ids.len());
    for reference_id in entry_css_reference_ids {
      match ctx.get_file_name(reference_id.as_str()) {
        Ok(file_name) => {
          entry_css_asset_file_names.insert(file_name);
        }
        _ => {
          // The asset was generated as part of a different output option.
          // It was already handled during the previous run of this plugin.
        }
      }
    }

    for file in args.bundle.iter() {
      match file {
        Output::Chunk(chunk) => {
          let name = self.get_chunk_name(chunk);
          let chunk_manifest = Arc::new(self.create_chunk(args.bundle, chunk, name.clone()));
          manifest.insert(name.clone(), chunk_manifest);
        }
        Output::Asset(asset) => {
          if !asset.names.is_empty() {
            let src = asset.original_file_names.first().map_or_else(
              || {
                format!(
                  "_{}",
                  Path::new(asset.filename.as_str())
                    .file_name()
                    .map(|x| x.to_string_lossy())
                    .unwrap()
                )
              },
              ToOwned::to_owned,
            );
            let is_entry = entry_css_asset_file_names.contains(asset.filename.as_str());
            let asset_manifest = Arc::new(Self::create_asset(asset, src.clone(), is_entry));

            // If JS chunk and asset chunk are both generated from the same source file,
            // prioritize JS chunk as it contains more information
            if !manifest.get(&src).is_some_and(|m| {
              m.file.ends_with(".js") || m.file.ends_with(".cjs") || m.file.ends_with(".mjs")
            }) {
              manifest.insert(src.clone(), Arc::<ManifestChunk>::clone(&asset_manifest));
            }

            for original_file_name in &asset.original_file_names {
              if !manifest.get(original_file_name).is_some_and(|m| {
                m.file.ends_with(".js") || m.file.ends_with(".cjs") || m.file.ends_with(".mjs")
              }) {
                manifest
                  .insert(original_file_name.clone(), Arc::<ManifestChunk>::clone(&asset_manifest));
              }
            }
          }
        }
      }
    }

    // TODO: uncomment these when multiple outputs are supported
    // output_count += 1;
    // let output = config.build.rollupOptions?.output
    // let outputLength = Array.isArray(output) ? output.length : 1
    // if output_count >= outputLength {
    ctx
      .emit_file_async(EmittedAsset {
        file_name: Some(self.config.out_path.as_str().into()),
        name: None,
        original_file_name: None,
        source: (serde_json::to_string_pretty(&manifest)?).into(),
      })
      .await?;
    // }

    Ok(())
  }
}

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
  fn get_chunk_name(&self, chunk: &OutputChunk) -> String {
    get_chunk_original_file_name(chunk, &self.config.root)
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
  fn create_chunk(&self, bundle: &Vec<Output>, chunk: &OutputChunk, src: String) -> ManifestChunk {
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
  match &chunk.facade_module_id {
    Some(facade_module_id) => {
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
    }
    _ => {
      format!("_{}", Path::new(chunk.filename.as_str()).file_name().unwrap().to_string_lossy())
    }
  }
}
