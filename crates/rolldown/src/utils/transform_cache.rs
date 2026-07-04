//! Persistent (filesystem) cache for per-module transform results.
//!
//! See `internal-docs/transform-cache/design.md` for the rationale and
//! `internal-docs/transform-cache/implementation.md` for the data flow.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{
  ModuleType, NormalizedBundlerOptions, PluginIdx, SourcemapChainElement,
  side_effects::HookSideEffects,
};
use rolldown_plugin::PluginDriver;
use rolldown_utils::xxhash::xxhash_with_base;

/// Bump whenever the on-disk entry layout changes. Mixed into the cache salt,
/// so old entries simply stop matching instead of failing to decode.
const FORMAT_VERSION: u8 = 1;
const MAGIC: [u8; 4] = *b"RDTC";
const HEADER_LEN: usize = MAGIC.len() + 1 + 8;

/// The portion of a module's scan state produced by the plugin `transform`
/// pipeline. On a cache hit this is everything needed to skip
/// [`crate::utils::transform_source::transform_source`] entirely.
pub struct CachedTransform {
  pub code: String,
  pub module_type: ModuleType,
  /// `Some` only when a transform hook overrode the side effects, so a hit
  /// never clobbers side effects derived from resolution or the load hook.
  pub side_effects: Option<HookSideEffects>,
  pub sourcemap_chain: Vec<SourcemapChainElement>,
}

/// A content-addressed store of [`CachedTransform`] entries: one file per
/// entry at `<dir>/<key[0..2]>/<key>`, where the key is a hash covering the
/// cache salt and the module's identity plus post-`load` source. The layout is
/// deliberately dumb so external tooling can sync it to and from remote
/// storage without understanding the entry format.
pub struct TransformCache {
  dir: PathBuf,
  salt: String,
}

impl TransformCache {
  pub fn new(
    options: &NormalizedBundlerOptions,
    plugin_driver: &PluginDriver,
  ) -> Option<Arc<Self>> {
    let cache_options = options.experimental.transform_cache_options()?;
    let dir = options
      .cwd
      .join(cache_options.dir.as_deref().unwrap_or("node_modules/.cache/rolldown"))
      .join("transform-v1");

    // Everything that invalidates the whole cache goes into the salt. Plugin
    // configurations and implementations are invisible here; callers fold
    // those into `key` (see `TransformCacheOptions::key`).
    let mut salt_input = vec![FORMAT_VERSION];
    salt_input.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    salt_input.push(0);
    salt_input.extend_from_slice(cache_options.key.as_deref().unwrap_or_default().as_bytes());
    for plugin in plugin_driver.plugins() {
      salt_input.push(0);
      salt_input.extend_from_slice(plugin.call_name().as_bytes());
    }
    let salt = xxhash_with_base(&salt_input, 16);

    Some(Arc::new(Self { dir, salt }))
  }

  pub fn cache_key(&self, stable_id: &str, module_type: &ModuleType, source: &str) -> String {
    // Hash the (potentially large) source separately so the combined input
    // stays small; `stable_id` keeps keys portable across machines.
    let source_hash = xxhash_with_base(source.as_bytes(), 16);
    let mut input = Vec::with_capacity(self.salt.len() + stable_id.len() + source_hash.len() + 16);
    input.extend_from_slice(self.salt.as_bytes());
    input.push(0);
    input.extend_from_slice(stable_id.as_bytes());
    input.push(0);
    input.extend_from_slice(module_type.to_string().as_bytes());
    input.push(0);
    input.extend_from_slice(source_hash.as_bytes());
    xxhash_with_base(&input, 16)
  }

  pub async fn get(&self, key: &str) -> Option<CachedTransform> {
    let path = self.entry_path(key);
    #[cfg(not(target_family = "wasm"))]
    {
      tokio::runtime::Handle::current()
        .spawn_blocking(move || read_entry(&path))
        .await
        .ok()
        .flatten()
    }
    #[cfg(target_family = "wasm")]
    {
      read_entry(&path)
    }
  }

  pub async fn set(
    &self,
    key: &str,
    code: &str,
    module_type: &ModuleType,
    side_effects: Option<HookSideEffects>,
    transform_chain: &[SourcemapChainElement],
  ) {
    let Some(bytes) = encode_entry(code, module_type, side_effects, transform_chain) else {
      return;
    };
    let path = self.entry_path(key);
    #[cfg(not(target_family = "wasm"))]
    {
      let _ =
        tokio::runtime::Handle::current().spawn_blocking(move || write_entry(&path, &bytes)).await;
    }
    #[cfg(target_family = "wasm")]
    {
      write_entry(&path, &bytes);
    }
  }

  fn entry_path(&self, key: &str) -> PathBuf {
    self.dir.join(&key[0..2]).join(key)
  }
}

fn side_effects_to_u8(side_effects: HookSideEffects) -> u8 {
  match side_effects {
    HookSideEffects::True => 0,
    HookSideEffects::False => 1,
    HookSideEffects::NoTreeshake => 2,
  }
}

fn side_effects_from_u8(value: u64) -> Option<HookSideEffects> {
  match value {
    0 => Some(HookSideEffects::True),
    1 => Some(HookSideEffects::False),
    2 => Some(HookSideEffects::NoTreeshake),
    _ => None,
  }
}

/// Entry layout: `MAGIC`, format version byte, `u64` LE metadata length, the
/// metadata JSON, then the raw transformed code bytes. Keeping the code out of
/// the JSON avoids escaping the biggest blob.
fn encode_entry(
  code: &str,
  module_type: &ModuleType,
  side_effects: Option<HookSideEffects>,
  transform_chain: &[SourcemapChainElement],
) -> Option<Vec<u8>> {
  let mut chain = Vec::with_capacity(transform_chain.len());
  for element in transform_chain {
    let value = match element {
      SourcemapChainElement::Transform((plugin_idx, map)) => serde_json::json!({
        "t": "map",
        "p": plugin_idx.raw(),
        "m": map.to_json_string(),
      }),
      SourcemapChainElement::Omitted { plugin_idx, plugin_name } => serde_json::json!({
        "t": "omitted",
        "p": plugin_idx.raw(),
        "n": plugin_name.as_str(),
      }),
      SourcemapChainElement::Null { plugin_idx, original_content } => serde_json::json!({
        "t": "null",
        "p": plugin_idx.raw(),
        "c": original_content.as_str(),
      }),
      // `Load` elements are produced before the transform pipeline; one showing
      // up here means the caller sliced the chain wrong. Don't cache garbage.
      SourcemapChainElement::Load(_) => return None,
    };
    chain.push(value);
  }
  let meta = serde_json::json!({
    "moduleType": module_type.to_string(),
    "sideEffects": side_effects.map(side_effects_to_u8),
    "chain": chain,
  });
  let meta_bytes = serde_json::to_vec(&meta).ok()?;

  let mut bytes = Vec::with_capacity(HEADER_LEN + meta_bytes.len() + code.len());
  bytes.extend_from_slice(&MAGIC);
  bytes.push(FORMAT_VERSION);
  bytes.extend_from_slice(&(meta_bytes.len() as u64).to_le_bytes());
  bytes.extend_from_slice(&meta_bytes);
  bytes.extend_from_slice(code.as_bytes());
  Some(bytes)
}

/// Any malformed or unreadable entry is treated as a miss; the build then
/// recomputes and rewrites it.
fn read_entry(path: &Path) -> Option<CachedTransform> {
  let bytes = std::fs::read(path).ok()?;
  decode_entry(&bytes)
}

fn decode_entry(bytes: &[u8]) -> Option<CachedTransform> {
  if bytes.len() < HEADER_LEN || bytes[0..4] != MAGIC || bytes[4] != FORMAT_VERSION {
    return None;
  }
  let meta_len = usize::try_from(u64::from_le_bytes(bytes[5..13].try_into().ok()?)).ok()?;
  let code_offset = HEADER_LEN.checked_add(meta_len)?;
  if bytes.len() < code_offset {
    return None;
  }
  let meta: serde_json::Value = serde_json::from_slice(&bytes[HEADER_LEN..code_offset]).ok()?;
  let code = String::from_utf8(bytes[code_offset..].to_vec()).ok()?;

  let module_type_str = meta.get("moduleType")?.as_str()?;
  let module_type = ModuleType::from_known_str(module_type_str)
    .unwrap_or_else(|_| ModuleType::Custom(module_type_str.to_string()));
  let side_effects = match meta.get("sideEffects")? {
    serde_json::Value::Null => None,
    value => Some(side_effects_from_u8(value.as_u64()?)?),
  };

  let mut sourcemap_chain = vec![];
  for element in meta.get("chain")?.as_array()? {
    let plugin_idx = PluginIdx::from_raw(u32::try_from(element.get("p")?.as_u64()?).ok()?);
    sourcemap_chain.push(match element.get("t")?.as_str()? {
      "map" => SourcemapChainElement::Transform((
        plugin_idx,
        // Parse through the lifetime-generic type; the stored chain needs the
        // owned (`'static`) form.
        oxc_sourcemap::SourceMap::from_json_string(element.get("m")?.as_str()?).ok()?.into_owned(),
      )),
      "omitted" => SourcemapChainElement::Omitted {
        plugin_idx,
        plugin_name: ArcStr::from(element.get("n")?.as_str()?),
      },
      "null" => SourcemapChainElement::Null {
        plugin_idx,
        original_content: ArcStr::from(element.get("c")?.as_str()?),
      },
      _ => return None,
    });
  }

  Some(CachedTransform { code, module_type, side_effects, sourcemap_chain })
}

/// Writes go to a process-unique temp file first and are moved into place with
/// a rename, so concurrent builds sharing a cache dir never observe partial
/// entries. All errors are swallowed: the cache is an optimization and must
/// never fail the build.
fn write_entry(path: &Path, bytes: &[u8]) {
  let Some(parent) = path.parent() else { return };
  if let Err(error) = std::fs::create_dir_all(parent) {
    tracing::debug!("failed to create transform cache dir {}: {error}", parent.display());
    return;
  }
  let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else { return };
  let tmp_path = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
  let result = std::fs::write(&tmp_path, bytes).and_then(|()| {
    std::fs::rename(&tmp_path, path).inspect_err(|_| {
      let _ = std::fs::remove_file(&tmp_path);
    })
  });
  if let Err(error) = result {
    tracing::debug!("failed to write transform cache entry {}: {error}", path.display());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn entry_roundtrip() {
    let chain = vec![
      SourcemapChainElement::Omitted {
        plugin_idx: PluginIdx::from_raw(3),
        plugin_name: "p".into(),
      },
      SourcemapChainElement::Null {
        plugin_idx: PluginIdx::from_raw(4),
        original_content: "const a = 1;".into(),
      },
    ];
    let bytes =
      encode_entry("const a = 2;", &ModuleType::Tsx, Some(HookSideEffects::False), &chain).unwrap();
    let entry = decode_entry(&bytes).unwrap();
    assert_eq!(entry.code, "const a = 2;");
    assert_eq!(entry.module_type, ModuleType::Tsx);
    assert_eq!(entry.side_effects, Some(HookSideEffects::False));
    assert_eq!(entry.sourcemap_chain.len(), 2);
  }

  #[test]
  fn rejects_unknown_version_and_garbage() {
    let bytes = encode_entry("code", &ModuleType::Js, None, &[]).unwrap();
    let mut wrong_version = bytes.clone();
    wrong_version[4] = FORMAT_VERSION + 1;
    assert!(decode_entry(&wrong_version).is_none());
    assert!(decode_entry(&bytes[0..HEADER_LEN - 1]).is_none());
    assert!(decode_entry(b"not a cache entry").is_none());
  }
}
