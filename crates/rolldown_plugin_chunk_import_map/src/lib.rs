use std::{
  borrow::Cow,
  hash::Hash,
  sync::atomic::{AtomicBool, Ordering},
};

use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, Output};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutputMap, HookUsage, Plugin};
use rolldown_utils::{
  dashmap::FxDashMap,
  hash_placeholder::{HASH_PLACEHOLDER_LEFT_FINDER, find_hash_placeholders},
  rustc_hash::FxHashMapExt as _,
  xxhash::encode_hash_with_base,
};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Default)]
pub struct ChunkImportMapPlugin {
  pub base_url: Option<String>,
  pub file_name: Option<String>,
  pub initialized: AtomicBool,
  pub chunk_import_map: FxDashMap<ArcStr, String>,
}

impl Plugin for ChunkImportMapPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:chunk-import-map")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk | HookUsage::GenerateBundle
  }

  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if !self.initialized.swap(true, Ordering::SeqCst) {
      let base = args.options.hash_characters.base();
      let mut used_names = FxHashSet::default();
      let mut used_chunk_ids = FxHashSet::default();
      for chunk in args.chunks.values() {
        let hash_placeholders: Vec<_> =
          find_hash_placeholders(&chunk.filename, &HASH_PLACEHOLDER_LEFT_FINDER).collect();
        if hash_placeholders.is_empty() {
          continue;
        }
        let hasher = match &chunk.facade_module_id {
          Some(module_id) => {
            let mut hasher = Xxh3::with_seed(0);
            module_id.as_str().hash(&mut hasher);
            hasher
          }
          None => {
            // Fallback logic for common chunk
            let mut hasher = Xxh3::with_seed(1);
            if used_names.contains(&chunk.name) {
              // Reduce the impact factor
              let Some(module_id) = chunk.module_ids.iter().min() else { continue };
              module_id.as_str().hash(&mut hasher);
            } else {
              used_names.insert(chunk.name.clone());
              chunk.name.hash(&mut hasher);
            }
            hasher
          }
        };
        let mut hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), base);
        let mut chunk_id = replace_hash_placeholders(&chunk.filename, &hash_placeholders, &hash);
        if used_chunk_ids.contains(&chunk_id) {
          // Chunks can share modules (`experimentalInlineCommonChunks` copies a record into every
          // carrier), so the fallback above can repeat. Hash every module, then a counter.
          let mut hasher = Xxh3::with_seed(2);
          chunk.name.hash(&mut hasher);
          let mut module_ids: Vec<_> = chunk.module_ids.iter().map(|id| id.as_str()).collect();
          module_ids.sort_unstable();
          module_ids.hash(&mut hasher);
          let mut attempt = 0u32;
          loop {
            hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), base);
            chunk_id = replace_hash_placeholders(&chunk.filename, &hash_placeholders, &hash);
            if !used_chunk_ids.contains(&chunk_id) {
              break;
            }
            attempt += 1;
            attempt.hash(&mut hasher);
          }
        }
        for &(start, end, placeholder) in &hash_placeholders {
          self.chunk_import_map.insert(placeholder.into(), hash[..end - start].to_string());
        }
        used_chunk_ids.insert(chunk_id.clone());
        self.chunk_import_map.insert(chunk.filename.clone(), chunk_id);
      }
    }

    let placeholders: Vec<_> = find_hash_placeholders(&args.code, &HASH_PLACEHOLDER_LEFT_FINDER)
      .filter(|placeholder| self.chunk_import_map.contains_key(placeholder.2))
      .collect();

    if placeholders.is_empty() {
      return Ok(None);
    }

    let mut code = args.code.as_ref().clone();
    for (start, end, placeholder) in placeholders {
      let hash = self.chunk_import_map.get(placeholder).expect("hash placeholder must exist");
      debug_assert_eq!(hash.len(), end - start, "hash length doesn't match placeholder size");
      unsafe {
        code.as_bytes_mut()[start..end].copy_from_slice(hash.as_bytes());
      }
    }
    Ok(Some(HookRenderChunkOutput { code, map: HookTransformOutputMap::Null }))
  }

  fn render_chunk_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::PinPost) })
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.chunk_import_map.is_empty() {
      return Ok(());
    }

    let base_url = self.base_url.as_deref().unwrap_or("/");
    let mut chunk_import_map = FxHashMap::with_capacity(self.chunk_import_map.len() / 2);
    for output in args.bundle.iter() {
      let Output::Chunk(chunk) = output else { continue };
      if let Some(v) = self.chunk_import_map.get(chunk.preliminary_filename.as_str()) {
        chunk_import_map.insert(
          rolldown_utils::concat_string!(base_url, v.as_str()),
          rolldown_utils::concat_string!(base_url, chunk.filename),
        );
      }
    }

    ctx
      .emit_file_async(EmittedAsset {
        file_name: Some(
          self.file_name.as_ref().map_or(arcstr::literal!("importmap.json"), ArcStr::from),
        ),
        source: (serde_json::to_string(&serde_json::json!({ "imports": chunk_import_map }))?)
          .into(),
        ..Default::default()
      })
      .await?;

    Ok(())
  }

  fn generate_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }
}

fn replace_hash_placeholders(
  filename: &str,
  hash_placeholders: &[(usize, usize, &str)],
  hash: &str,
) -> String {
  let mut chunk_id = filename.to_string();
  for &(start, end, _) in hash_placeholders {
    unsafe { chunk_id.as_bytes_mut()[start..end].copy_from_slice(&hash.as_bytes()[..end - start]) };
  }
  chunk_id
}
