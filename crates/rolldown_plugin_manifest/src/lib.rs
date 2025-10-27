mod utils;

use std::{borrow::Cow, collections::BTreeMap, path::Path, pin::Pin, sync::Arc};

use rolldown_common::{EmittedAsset, Output};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashSetExt;
use rustc_hash::FxHashSet;

pub type IsLegacyFn =
  dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>> + Send + Sync;

pub type CssEntriesFn =
  dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<FxHashSet<String>>> + Send>> + Send + Sync;

#[derive(derive_more::Debug)]
pub struct ManifestPlugin {
  pub root: String,
  pub out_path: String,
  #[debug(skip)]
  pub is_legacy: Option<Arc<IsLegacyFn>>,
  #[debug(skip)]
  pub css_entries: Arc<CssEntriesFn>,
}

impl Plugin for ManifestPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:manifest")
  }

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    let is_legacy = match &self.is_legacy {
      Some(is_legacy_fn) => is_legacy_fn().await?,
      None => false,
    };

    // Use BTreeMap to make the result sorted
    let mut manifest = BTreeMap::default();
    let mut css_entries = None;

    for file in args.bundle.iter() {
      match file {
        Output::Chunk(chunk) => {
          let name = self.get_chunk_name(chunk, is_legacy);
          let chunk_manifest = Arc::new(self.create_chunk(args.bundle, chunk, &name, is_legacy));
          manifest.insert(name, chunk_manifest);
        }
        Output::Asset(asset) => {
          if !asset.names.is_empty() {
            if css_entries.is_none() {
              let reference_ids = (self.css_entries)().await?;
              let mut filenames = FxHashSet::with_capacity(reference_ids.len());
              for reference_id in reference_ids {
                if let Ok(filename) = ctx.get_file_name(&reference_id) {
                  filenames.insert(filename);
                }
              }
              css_entries = Some(filenames);
            }

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

            let css_entries = unsafe { css_entries.as_ref().unwrap_unchecked() };
            let asset_manifest = Arc::new(Self::create_asset(
              asset,
              file.to_string(),
              css_entries.contains(&asset.filename),
            ));

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
        file_name: Some(self.out_path.as_str().into()),
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
