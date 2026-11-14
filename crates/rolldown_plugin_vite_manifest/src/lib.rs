mod utils;

use std::{borrow::Cow, collections::BTreeMap, path::Path, pin::Pin, sync::Arc};

use rolldown_common::{EmittedAsset, NormalizedBundlerOptions, Output};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashMapExt as _;
use rustc_hash::FxHashMap;

pub type IsLegacyFn = dyn Fn(&Arc<NormalizedBundlerOptions>) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>>
  + Send
  + Sync;

pub type CssEntriesFn = dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<FxHashMap<String, String>>> + Send>>
  + Send
  + Sync;

#[derive(derive_more::Debug)]
pub struct ViteManifestPlugin {
  pub root: String,
  pub out_path: String,
  #[debug(skip)]
  pub is_legacy: Option<Arc<IsLegacyFn>>,
  #[debug(skip)]
  pub css_entries: Arc<CssEntriesFn>,
}

impl Plugin for ViteManifestPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-manifest")
  }

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    let is_legacy = match &self.is_legacy {
      Some(is_legacy_fn) => is_legacy_fn(args.options).await?,
      None => false,
    };

    // Use BTreeMap to make the result sorted
    let mut manifest = BTreeMap::default();
    let mut css_entries = None;

    // Manifest name of each chunk by file name, computed once. Resolving `imports` and
    // `dynamicImports` used to search the whole bundle for every import, O(chunks * imports).
    // The first chunk with a given file name wins, as with the search it replaces.
    let mut chunk_names: FxHashMap<&str, String> = FxHashMap::default();
    for file in args.bundle.iter() {
      if let Output::Chunk(chunk) = file {
        chunk_names
          .entry(chunk.filename.as_str())
          .or_insert_with(|| self.get_chunk_name(chunk, is_legacy));
      }
    }

    for file in args.bundle.iter() {
      match file {
        Output::Chunk(chunk) => {
          let name = chunk_names[chunk.filename.as_str()].clone();
          let chunk_manifest = Arc::new(Self::create_chunk(&chunk_names, chunk, &name));
          manifest.insert(name, chunk_manifest);
        }
        Output::Asset(asset) => {
          if !asset.names.is_empty() {
            if css_entries.is_none() {
              let reference_ids = (self.css_entries)().await?;
              let mut filenames = FxHashMap::with_capacity(reference_ids.len());
              for (reference_id, name) in reference_ids {
                if let Ok(filename) = ctx.get_file_name(&reference_id) {
                  filenames.insert(filename, name);
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
              css_entries.get(&asset.filename).map(ToString::to_string),
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
