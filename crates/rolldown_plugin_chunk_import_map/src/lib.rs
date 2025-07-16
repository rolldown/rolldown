use std::{
  borrow::Cow,
  hash::Hash,
  sync::atomic::{AtomicBool, Ordering},
};

use arcstr::ArcStr;
use rolldown_plugin::{HookRenderChunkOutput, HookUsage, Plugin};
use rolldown_utils::{
  dashmap::FxDashMap,
  hash_placeholder::{find_hash_placeholders, hash_placeholder_left_finder},
  xxhash::xxhash_with_base,
};
use rustc_hash::FxHashSet;
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Default)]
pub struct ChunkImportMapPlugin {
  initialized: AtomicBool,
  chunk_import_map: FxDashMap<ArcStr, String>,
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
    let hash_finder = hash_placeholder_left_finder();
    if !self.initialized.swap(true, Ordering::SeqCst) {
      let base = args.options.hash_characters.base();
      let mut used_names = FxHashSet::default();
      for chunk in args.chunks.values() {
        let hash_placeholders = find_hash_placeholders(&chunk.filename, &hash_finder);
        if hash_placeholders.is_empty() {
          continue;
        }
        let hasher = match &chunk.facade_module_id {
          Some(module_id) => {
            let mut hasher = Xxh3::with_seed(0);
            module_id.resource_id().as_str().hash(&mut hasher);
            hasher
          }
          None => {
            // Fallback logic for common chunk
            let mut hasher = Xxh3::with_seed(1);
            if used_names.contains(&chunk.name) {
              // Reduce the impact factor
              let Some(module_id) = chunk.module_ids.iter().min() else { continue };
              module_id.resource_id().as_str().hash(&mut hasher);
            } else {
              used_names.insert(chunk.name.clone());
              chunk.name.hash(&mut hasher);
            }
            hasher
          }
        };
        let hash = xxhash_with_base(&hasher.digest128().to_le_bytes(), base);
        let mut chunk_id = chunk.filename.to_string();
        for (start, end, placeholder) in hash_placeholders {
          let hash = hash[..end - start].to_string();
          unsafe { chunk_id.as_bytes_mut()[start..end].copy_from_slice(hash.as_bytes()) };
          self.chunk_import_map.insert(placeholder.into(), hash);
        }
        self.chunk_import_map.insert(chunk.filename.clone(), chunk_id);
      }
    }

    let mut placeholders = find_hash_placeholders(&args.code, &hash_finder);
    placeholders.retain(|placeholder| self.chunk_import_map.contains_key(placeholder.2));

    if placeholders.is_empty() {
      return Ok(None);
    }

    let mut code = args.code.clone();
    for (start, end, placeholder) in placeholders {
      let hash = self.chunk_import_map.get(placeholder).expect("hash placeholder must exist");
      debug_assert_eq!(hash.len(), end - start, "hash length doesn't match placeholder size");
      unsafe {
        code.as_bytes_mut()[start..end].copy_from_slice(hash.as_bytes());
      }
    }
    Ok(Some(HookRenderChunkOutput { code, map: None }))
  }

  fn render_chunk_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Post) })
  }

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    Ok(())
  }

  fn generate_bundle_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }
}
