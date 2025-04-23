mod utils;

use std::{borrow::Cow, collections::BTreeMap, path::Path, sync::Arc};

use rolldown_common::{EmittedAsset, Output};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::FxHashSet;
use utils::ManifestChunk;

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

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::GenerateBundle
  }
}
