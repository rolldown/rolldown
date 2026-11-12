use std::{
  borrow::Cow,
  hash::Hash,
  sync::{Arc, OnceLock},
};

use arcstr::ArcStr;
use rolldown_common::{ChunkHashDependencies, EmittedAsset, Output, RollupRenderedChunk};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutputMap, HookUsage, Plugin};
use rolldown_utils::{
  hash_placeholder::{
    HASH_PLACEHOLDER_LEFT_FINDER, find_hash_placeholders, visit_with_placeholders_defaulted,
  },
  indexmap::FxIndexMap,
  pattern_filter::StringOrRegex,
  rustc_hash::FxHashMapExt as _,
  xxhash::encode_hash_with_base,
};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

#[derive(Debug, Default)]
pub struct ChunkImportMapPlugin {
  pub base_url: Option<String>,
  pub file_name: Option<String>,
  pub include: Option<Vec<StringOrRegex>>,
}

/// Lives in the plugin context meta, which is created for each build.
#[derive(Debug, Default)]
struct ChunkImportMapState(OnceLock<StableChunkNames>);

#[derive(Debug, Default)]
struct StableChunkNames {
  /// Hash placeholder to stable hash.
  hashes: FxHashMap<String, String>,
  /// Preliminary file name to stable file name.
  filenames: FxHashMap<ArcStr, String>,
}

fn is_hashed(filename: &str) -> bool {
  find_hash_placeholders(filename, &HASH_PLACEHOLDER_LEFT_FINDER).next().is_some()
}

impl ChunkImportMapPlugin {
  fn is_included(&self, chunk_name: &str) -> bool {
    self.include.as_ref().is_none_or(|patterns| {
      patterns.iter().any(|pattern| match pattern {
        StringOrRegex::String(name) => name == chunk_name,
        StringOrRegex::Regex(regex) => regex.matches(chunk_name),
      })
    })
  }

  fn collect_stable_names(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> StableChunkNames {
    let mut stable_names = StableChunkNames::default();
    let base = args.options.hash_characters.base();
    let mut used_names = FxHashSet::default();
    for chunk in args.chunks.values() {
      let mut hash_placeholders =
        find_hash_placeholders(&chunk.filename, &HASH_PLACEHOLDER_LEFT_FINDER).peekable();
      if hash_placeholders.peek().is_none() || !self.is_included(&chunk.name) {
        continue;
      }
      let hasher = match &chunk.facade_module_id {
        Some(module_id) => {
          let mut hasher = Xxh3::with_seed(0);
          let is_path = module_id.is_path();
          if is_path {
            (is_path, module_id.stabilize(&args.options.cwd).as_str()).hash(&mut hasher);
          } else {
            (is_path, module_id.as_str()).hash(&mut hasher);
          }
          hasher
        }
        None => {
          let mut hasher = Xxh3::with_seed(1);
          if used_names.contains(&chunk.name) {
            // See internal-docs/inline-common-chunks/implementation.md.
            let logical_chunk = &args.chunk_table[chunk.originate_from];
            let Some(module_key) = logical_chunk
              .modules
              .iter()
              .map(|&idx| {
                let module = &args.module_table[idx];
                let is_path = module.id().is_path();
                let id = if is_path { module.stable_id().as_str() } else { module.id().as_str() };
                (is_path, id)
              })
              .min()
            else {
              continue;
            };
            module_key.hash(&mut hasher);
          } else {
            used_names.insert(chunk.name.clone());
            chunk.name.hash(&mut hasher);
          }
          hasher
        }
      };
      let hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), base);
      let mut chunk_id = chunk.filename.to_string();
      for (start, end, placeholder) in hash_placeholders {
        let hash = hash[..end - start].to_string();
        unsafe { chunk_id.as_bytes_mut()[start..end].copy_from_slice(hash.as_bytes()) };
        stable_names.hashes.insert(placeholder.to_string(), hash);
      }
      stable_names.filenames.insert(chunk.filename.clone(), chunk_id);
    }
    let hash_dependencies = collect_hash_dependencies(&args.chunks, &stable_names.filenames);
    if !hash_dependencies.is_empty() {
      ctx.meta().insert(Arc::new(ChunkHashDependencies(hash_dependencies)));
    }
    stable_names
  }
}

/// A chunk outside the map is referenced by its hashed file name. For every chunk, collects those
/// file names from the static import closures of its dynamic import targets.
fn collect_hash_dependencies(
  chunks: &FxIndexMap<ArcStr, Arc<RollupRenderedChunk>>,
  mapped: &FxHashMap<ArcStr, String>,
) -> FxHashMap<ArcStr, String> {
  let is_outside_map = |filename: &str| is_hashed(filename) && !mapped.contains_key(filename);
  let mut hash_dependencies = FxHashMap::default();
  if !chunks.keys().any(|filename| is_outside_map(filename)) {
    return hash_dependencies;
  }

  let mut loaded_with: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
  for target in chunks.values().flat_map(|chunk| &chunk.dynamic_imports) {
    let Some(chunk) = chunks.get(target) else { continue };
    if loaded_with.contains_key(target.as_str()) {
      continue;
    }
    let mut outside_map = vec![];
    let mut visited = FxHashSet::default();
    let mut stack = vec![chunk];
    while let Some(chunk) = stack.pop() {
      for import in &chunk.imports {
        if !visited.insert(import.as_str()) {
          continue;
        }
        if is_outside_map(import) {
          outside_map.push(import.as_str());
        }
        if let Some(chunk) = chunks.get(import) {
          stack.push(chunk);
        }
      }
    }
    loaded_with.insert(target.as_str(), outside_map);
  }

  for chunk in chunks.values() {
    // The order must not depend on the indices of the placeholders.
    let mut filenames = chunk
      .dynamic_imports
      .iter()
      .filter_map(|target| loaded_with.get(target.as_str()))
      .flatten()
      .map(|filename| {
        let mut normalized = vec![];
        visit_with_placeholders_defaulted(
          filename,
          &HASH_PLACEHOLDER_LEFT_FINDER,
          |_| true,
          |bytes| normalized.extend_from_slice(bytes),
        );
        (normalized, *filename)
      })
      .collect::<Vec<_>>();
    if filenames.is_empty() {
      continue;
    }
    filenames.sort_unstable();
    filenames.dedup();
    let filenames = filenames.into_iter().map(|(_, filename)| filename).collect::<Vec<_>>();
    hash_dependencies.insert(chunk.filename.clone(), filenames.join(","));
  }
  hash_dependencies
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
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let state = ctx.meta().get_or_insert_default::<ChunkImportMapState>();
    let stable_names = state.0.get_or_init(|| self.collect_stable_names(ctx, args));

    let placeholders: Vec<_> = find_hash_placeholders(&args.code, &HASH_PLACEHOLDER_LEFT_FINDER)
      .filter(|placeholder| stable_names.hashes.contains_key(placeholder.2))
      .collect();

    if placeholders.is_empty() {
      return Ok(None);
    }

    let mut code = args.code.as_ref().clone();
    for (start, end, placeholder) in placeholders {
      let hash = &stable_names.hashes[placeholder];
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
    let state = ctx.meta().get::<ChunkImportMapState>();
    let Some(stable_names) = state.as_ref().and_then(|state| state.0.get()) else {
      return Ok(());
    };
    if stable_names.filenames.is_empty() {
      return Ok(());
    }

    let base_url = self.base_url.as_deref().unwrap_or("/");
    let mut chunk_import_map = FxHashMap::with_capacity(stable_names.filenames.len());
    for output in args.bundle.iter() {
      let Output::Chunk(chunk) = output else { continue };
      if let Some(v) = stable_names.filenames.get(chunk.preliminary_filename.as_str()) {
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
