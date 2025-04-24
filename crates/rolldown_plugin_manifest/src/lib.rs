mod utils;

use std::{borrow::Cow, collections::BTreeMap, path::Path, sync::Arc};

use rolldown_common::{EmittedAsset, Output};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::FxHashSet;

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

    let entry_css_reference_ids = &self.entry_css_asset_file_names;
    let mut entry_css_asset_file_names = FxHashSet::with_capacity(entry_css_reference_ids.len());

    for reference_id in entry_css_reference_ids {
      match ctx.get_file_name(reference_id) {
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
          let chunk_manifest = Arc::new(self.create_chunk(args.bundle, chunk, &name));
          manifest.insert(name, chunk_manifest);
        }
        Output::Asset(asset) => {
          if !asset.names.is_empty() {
            // Add every unique asset to the manifest, keyed by its original name
            let file = asset.original_file_names.first().map_or_else(
              || {
                Cow::Owned(rolldown_utils::concat_string!(
                  "_",
                  Path::new(asset.filename.as_str()).file_name().unwrap().to_string_lossy()
                ))
              },
              Cow::Borrowed,
            );

            let is_entry = entry_css_asset_file_names.contains(&asset.filename);
            let asset_manifest = Arc::new(Self::create_asset(asset, file.to_string(), is_entry));

            // If JS chunk and asset chunk are both generated from the same source file,
            // prioritize JS chunk as it contains more information
            if utils::is_non_js_file(&file, &manifest) {
              manifest.insert(file.into_owned(), Arc::clone(&asset_manifest));
            }

            for original_file_name in &asset.original_file_names {
              if utils::is_non_js_file(original_file_name, &manifest) {
                manifest.insert(original_file_name.clone(), Arc::clone(&asset_manifest));
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
        source: (serde_json::to_string_pretty(&manifest)?).into(),
        ..Default::default()
      })
      .await?;
    // }

    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::GenerateBundle
  }
}
