//! Persistent (filesystem) cache for per-module build pipeline results:
//! `load` hooks, `transform` hooks and dependency resolution.
//!
//! See `internal-docs/build-cache/design.md` for the rationale and
//! `internal-docs/build-cache/implementation.md` for the data flow.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  ImportRecordIdx, ModuleDefFormat, ModuleType, NormalizedBundlerOptions, PackageJson, PluginIdx,
  ResolvedExternal, ResolvedId, SourcemapChainElement,
  side_effects::{HookSideEffects, SideEffects},
};
use rolldown_plugin::PluginDriver;
use rolldown_utils::{stabilize_id::stabilize_id, xxhash::xxhash_with_base};
use sugar_path::SugarPath;

/// Bump whenever the on-disk entry layout changes. Mixed into the cache salt,
/// so old entries simply stop matching instead of failing to decode.
const FORMAT_VERSION: u8 = 1;
const MAGIC: [u8; 4] = *b"RDBC";
const HEADER_LEN: usize = MAGIC.len() + 1 + 8;

/// The portion of a module's scan state produced by the plugin `load` and
/// `transform` pipelines plus dependency resolution. On a cache hit this is
/// everything needed to skip all three for the module; parsing and scanning
/// re-run natively on `code` and deterministically reproduce the import
/// records `resolved_deps` is positionally aligned with.
pub struct CachedModule {
  pub code: String,
  pub module_type: ModuleType,
  /// `Some` only when a load or transform hook overrode the side effects, so
  /// a hit never clobbers side effects derived from resolution data that sits
  /// outside the cache key.
  pub side_effects: Option<HookSideEffects>,
  pub sourcemap_chain: Vec<SourcemapChainElement>,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
}

/// A content-addressed store of [`CachedModule`] entries: one file per entry
/// at `<dir>/<key[0..2]>/<key>`, where the key is a hash covering the cache
/// salt and the module's identity plus on-disk content. The layout is
/// deliberately dumb so external tooling can sync it to and from remote
/// storage without understanding the entry format.
pub struct BuildCache {
  dir: PathBuf,
  salt: String,
  cwd: PathBuf,
}

impl BuildCache {
  pub fn new(
    options: &NormalizedBundlerOptions,
    plugin_driver: &PluginDriver,
  ) -> Option<Arc<Self>> {
    let cache_options = options.experimental.build_cache_options()?;
    let dir = options
      .cwd
      .join(cache_options.dir.as_deref().unwrap_or("node_modules/.cache/rolldown"))
      .join("build-v1");

    // Everything that invalidates the whole cache goes into the salt. Plugin
    // configurations and implementations are invisible here; callers fold
    // those into `key` (see `BuildCacheOptions::key`).
    let mut salt_input = vec![FORMAT_VERSION];
    salt_input.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    salt_input.push(0);
    salt_input.extend_from_slice(cache_options.key.as_deref().unwrap_or_default().as_bytes());
    salt_input.push(0);
    salt_input.extend_from_slice(format!("{:?}", options.platform).as_bytes());
    // `moduleTypes` decides module types before the cached seam; sorted for a
    // deterministic hash across processes.
    let mut module_types: Vec<_> = options.module_types.iter().collect();
    module_types.sort_by_key(|(a, _)| *a);
    for (extension, module_type) in module_types {
      salt_input.push(0);
      salt_input.extend_from_slice(extension.as_bytes());
      salt_input.push(0);
      salt_input.extend_from_slice(module_type.to_string().as_bytes());
    }
    for plugin in plugin_driver.plugins() {
      salt_input.push(0);
      salt_input.extend_from_slice(plugin.call_name().as_bytes());
    }
    let salt = xxhash_with_base(&salt_input, 16);

    Some(Arc::new(Self { dir, salt, cwd: options.cwd.clone() }))
  }

  pub fn cache_key(
    &self,
    stable_id: &str,
    asserted_module_type: Option<&ModuleType>,
    source: &[u8],
  ) -> String {
    // Hash the (potentially large) source separately so the combined input
    // stays small; `stable_id` keeps keys portable across machines.
    let source_hash = xxhash_with_base(source, 16);
    let asserted = asserted_module_type.map(ToString::to_string).unwrap_or_default();
    let mut input = Vec::with_capacity(
      self.salt.len() + stable_id.len() + asserted.len() + source_hash.len() + 16,
    );
    input.extend_from_slice(self.salt.as_bytes());
    input.push(0);
    input.extend_from_slice(stable_id.as_bytes());
    input.push(0);
    input.extend_from_slice(asserted.as_bytes());
    input.push(0);
    input.extend_from_slice(source_hash.as_bytes());
    xxhash_with_base(&input, 16)
  }

  pub async fn get(&self, key: &str) -> Option<CachedModule> {
    let path = self.entry_path(key);
    let cwd = self.cwd.clone();
    #[cfg(not(target_family = "wasm"))]
    {
      tokio::runtime::Handle::current()
        .spawn_blocking(move || read_entry(&path, &cwd))
        .await
        .ok()
        .flatten()
    }
    #[cfg(target_family = "wasm")]
    {
      read_entry(&path, &cwd)
    }
  }

  pub async fn set(
    &self,
    key: &str,
    code: &str,
    module_type: &ModuleType,
    side_effects: Option<HookSideEffects>,
    sourcemap_chain: &[SourcemapChainElement],
    resolved_deps: &IndexVec<ImportRecordIdx, ResolvedId>,
  ) {
    let Some(bytes) =
      encode_entry(code, module_type, side_effects, sourcemap_chain, resolved_deps, &self.cwd)
    else {
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

fn hook_side_effects_to_u8(side_effects: HookSideEffects) -> u8 {
  match side_effects {
    HookSideEffects::True => 0,
    HookSideEffects::False => 1,
    HookSideEffects::NoTreeshake => 2,
  }
}

fn hook_side_effects_from_u64(value: u64) -> Option<HookSideEffects> {
  match value {
    0 => Some(HookSideEffects::True),
    1 => Some(HookSideEffects::False),
    2 => Some(HookSideEffects::NoTreeshake),
    _ => None,
  }
}

fn module_def_format_to_u8(format: ModuleDefFormat) -> u8 {
  match format {
    ModuleDefFormat::Unknown => 0,
    ModuleDefFormat::Cjs => 1,
    ModuleDefFormat::Cts => 2,
    ModuleDefFormat::CjsPackageJson => 3,
    ModuleDefFormat::EsmMjs => 4,
    ModuleDefFormat::EsmMts => 5,
    ModuleDefFormat::EsmPackageJson => 6,
  }
}

fn module_def_format_from_u64(value: u64) -> Option<ModuleDefFormat> {
  match value {
    0 => Some(ModuleDefFormat::Unknown),
    1 => Some(ModuleDefFormat::Cjs),
    2 => Some(ModuleDefFormat::Cts),
    3 => Some(ModuleDefFormat::CjsPackageJson),
    4 => Some(ModuleDefFormat::EsmMjs),
    5 => Some(ModuleDefFormat::EsmMts),
    6 => Some(ModuleDefFormat::EsmPackageJson),
    _ => None,
  }
}

fn resolved_external_to_u8(external: ResolvedExternal) -> u8 {
  match external {
    ResolvedExternal::Bool(false) => 0,
    ResolvedExternal::Bool(true) => 1,
    ResolvedExternal::Absolute => 2,
    ResolvedExternal::Relative => 3,
  }
}

fn resolved_external_from_u64(value: u64) -> Option<ResolvedExternal> {
  match value {
    0 => Some(ResolvedExternal::Bool(false)),
    1 => Some(ResolvedExternal::Bool(true)),
    2 => Some(ResolvedExternal::Absolute),
    3 => Some(ResolvedExternal::Relative),
    _ => None,
  }
}

/// Absolute paths are stored relative to `cwd` (marked with `abs`) so entries
/// stay portable across machines sharing the cache.
fn encode_path(path: &str, cwd: &Path) -> serde_json::Value {
  let is_absolute = Path::new(path).is_absolute();
  serde_json::json!({
    "p": if is_absolute { stabilize_id(path, cwd) } else { path.to_string() },
    "abs": is_absolute,
  })
}

fn decode_path(value: &serde_json::Value, cwd: &Path) -> Option<String> {
  let stored = value.get("p")?.as_str()?;
  if value.get("abs")?.as_bool()? {
    Some(stored.as_path().absolutize_with(cwd).to_str()?.to_string())
  } else {
    Some(stored.to_string())
  }
}

fn encode_side_effects(side_effects: &SideEffects) -> serde_json::Value {
  match side_effects {
    SideEffects::Bool(value) => serde_json::json!({ "b": value }),
    SideEffects::String(value) => serde_json::json!({ "s": value }),
    SideEffects::Array(values) => serde_json::json!({ "a": values }),
  }
}

fn decode_side_effects(value: &serde_json::Value) -> Option<SideEffects> {
  if let Some(b) = value.get("b") {
    Some(SideEffects::Bool(b.as_bool()?))
  } else if let Some(s) = value.get("s") {
    Some(SideEffects::String(s.as_str()?.to_string()))
  } else {
    let values = value.get("a")?.as_array()?;
    Some(SideEffects::Array(
      values.iter().map(|v| v.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
    ))
  }
}

fn encode_resolved_dep(dep: &ResolvedId, cwd: &Path) -> serde_json::Value {
  serde_json::json!({
    "id": encode_path(dep.id.as_str(), cwd),
    "format": module_def_format_to_u8(dep.module_def_format),
    "external": resolved_external_to_u8(dep.external),
    "normalizeExternalId": dep.normalize_external_id,
    "sideEffects": dep.side_effects.map(hook_side_effects_to_u8),
    "noSideEffects": dep.is_external_without_side_effects,
    "packageJson": dep.package_json.as_ref().map(|package_json| {
      serde_json::json!({
        "name": package_json.name(),
        "version": package_json.version(),
        "type": package_json.r#type(),
        "sideEffects": package_json.side_effects.as_ref().map(encode_side_effects),
        "realpath": encode_path(&package_json.realpath().to_string_lossy(), cwd),
      })
    }),
  })
}

fn decode_resolved_dep(value: &serde_json::Value, cwd: &Path) -> Option<ResolvedId> {
  let package_json = match value.get("packageJson")? {
    serde_json::Value::Null => None,
    package_json => {
      let r#type = match package_json.get("type")? {
        serde_json::Value::Null => None,
        value => Some(match value.as_str()? {
          "commonjs" => "commonjs",
          "module" => "module",
          _ => return None,
        }),
      };
      let side_effects = match package_json.get("sideEffects")? {
        serde_json::Value::Null => None,
        value => Some(decode_side_effects(value)?),
      };
      Some(Arc::new(PackageJson::from_parts(
        package_json.get("name")?.as_str().map(ArcStr::from),
        package_json.get("version")?.as_str().map(ArcStr::from),
        r#type,
        side_effects,
        PathBuf::from(decode_path(package_json.get("realpath")?, cwd)?),
      )))
    }
  };
  Some(ResolvedId {
    id: decode_path(value.get("id")?, cwd)?.into(),
    module_def_format: module_def_format_from_u64(value.get("format")?.as_u64()?)?,
    external: resolved_external_from_u64(value.get("external")?.as_u64()?)?,
    normalize_external_id: match value.get("normalizeExternalId")? {
      serde_json::Value::Null => None,
      value => Some(value.as_bool()?),
    },
    package_json,
    side_effects: match value.get("sideEffects")? {
      serde_json::Value::Null => None,
      value => Some(hook_side_effects_from_u64(value.as_u64()?)?),
    },
    is_external_without_side_effects: value.get("noSideEffects")?.as_bool()?,
  })
}

/// Entry layout: `MAGIC`, format version byte, `u64` LE metadata length, the
/// metadata JSON, then the raw transformed code bytes. Keeping the code out of
/// the JSON avoids escaping the biggest blob.
fn encode_entry(
  code: &str,
  module_type: &ModuleType,
  side_effects: Option<HookSideEffects>,
  sourcemap_chain: &[SourcemapChainElement],
  resolved_deps: &IndexVec<ImportRecordIdx, ResolvedId>,
  cwd: &Path,
) -> Option<Vec<u8>> {
  let mut chain = Vec::with_capacity(sourcemap_chain.len());
  for element in sourcemap_chain {
    let value = match element {
      SourcemapChainElement::Load(map) => serde_json::json!({
        "t": "load",
        "m": map.to_json_string(),
      }),
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
    };
    chain.push(value);
  }
  let meta = serde_json::json!({
    "moduleType": module_type.to_string(),
    "sideEffects": side_effects.map(hook_side_effects_to_u8),
    "chain": chain,
    "deps": resolved_deps.iter().map(|dep| encode_resolved_dep(dep, cwd)).collect::<Vec<_>>(),
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
/// recomputes and rewrites it. An entry whose resolved dependencies no longer
/// exist on disk is also a miss, so deleted or moved files fall back to a
/// fresh resolution instead of replaying a stale one.
fn read_entry(path: &Path, cwd: &Path) -> Option<CachedModule> {
  let bytes = std::fs::read(path).ok()?;
  let entry = decode_entry(&bytes, cwd)?;
  let deps_exist = entry.resolved_deps.iter().all(|dep| {
    let path = Path::new(dep.id.as_str());
    dep.external.is_external() || !path.is_absolute() || std::fs::metadata(path).is_ok()
  });
  deps_exist.then_some(entry)
}

fn decode_entry(bytes: &[u8], cwd: &Path) -> Option<CachedModule> {
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
    value => Some(hook_side_effects_from_u64(value.as_u64()?)?),
  };

  let mut sourcemap_chain = vec![];
  for element in meta.get("chain")?.as_array()? {
    sourcemap_chain.push(match element.get("t")?.as_str()? {
      // Parse through the lifetime-generic type; the stored chain needs the
      // owned (`'static`) form.
      "load" => SourcemapChainElement::Load(
        oxc_sourcemap::SourceMap::from_json_string(element.get("m")?.as_str()?).ok()?.into_owned(),
      ),
      "map" => SourcemapChainElement::Transform((
        decode_plugin_idx(element)?,
        oxc_sourcemap::SourceMap::from_json_string(element.get("m")?.as_str()?).ok()?.into_owned(),
      )),
      "omitted" => SourcemapChainElement::Omitted {
        plugin_idx: decode_plugin_idx(element)?,
        plugin_name: ArcStr::from(element.get("n")?.as_str()?),
      },
      "null" => SourcemapChainElement::Null {
        plugin_idx: decode_plugin_idx(element)?,
        original_content: ArcStr::from(element.get("c")?.as_str()?),
      },
      _ => return None,
    });
  }

  let mut resolved_deps = IndexVec::new();
  for dep in meta.get("deps")?.as_array()? {
    resolved_deps.push(decode_resolved_dep(dep, cwd)?);
  }

  Some(CachedModule { code, module_type, side_effects, sourcemap_chain, resolved_deps })
}

fn decode_plugin_idx(element: &serde_json::Value) -> Option<PluginIdx> {
  Some(PluginIdx::from_raw(u32::try_from(element.get("p")?.as_u64()?).ok()?))
}

/// Writes go to a process-unique temp file first and are moved into place with
/// a rename, so concurrent builds sharing a cache dir never observe partial
/// entries. All errors are swallowed: the cache is an optimization and must
/// never fail the build.
fn write_entry(path: &Path, bytes: &[u8]) {
  let Some(parent) = path.parent() else { return };
  if let Err(error) = std::fs::create_dir_all(parent) {
    tracing::debug!("failed to create build cache dir {}: {error}", parent.display());
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
    tracing::debug!("failed to write build cache entry {}: {error}", path.display());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn entry_roundtrip() {
    let cwd = std::env::current_dir().unwrap();
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
    let dep_path = cwd.join("dep.js");
    let mut deps: IndexVec<ImportRecordIdx, ResolvedId> = IndexVec::new();
    deps.push(ResolvedId {
      id: dep_path.to_str().unwrap().into(),
      module_def_format: ModuleDefFormat::EsmMjs,
      external: ResolvedExternal::Bool(false),
      normalize_external_id: None,
      package_json: Some(Arc::new(PackageJson::from_parts(
        Some("pkg".into()),
        Some("1.0.0".into()),
        Some("module"),
        Some(SideEffects::Array(vec!["*.css".to_string()])),
        cwd.join("package.json"),
      ))),
      side_effects: Some(HookSideEffects::False),
      is_external_without_side_effects: false,
    });
    deps.push(ResolvedId::new_external_without_side_effects("react".into()));

    let bytes = encode_entry(
      "const a = 2;",
      &ModuleType::Tsx,
      Some(HookSideEffects::False),
      &chain,
      &deps,
      &cwd,
    )
    .unwrap();
    let entry = decode_entry(&bytes, &cwd).unwrap();
    assert_eq!(entry.code, "const a = 2;");
    assert_eq!(entry.module_type, ModuleType::Tsx);
    assert_eq!(entry.side_effects, Some(HookSideEffects::False));
    assert_eq!(entry.sourcemap_chain.len(), 2);
    assert_eq!(entry.resolved_deps.len(), 2);

    let dep = &entry.resolved_deps[ImportRecordIdx::from_raw(0)];
    assert_eq!(dep.id.as_str(), dep_path.to_str().unwrap());
    assert_eq!(dep.module_def_format, ModuleDefFormat::EsmMjs);
    assert!(!dep.external.is_external());
    assert_eq!(dep.side_effects, Some(HookSideEffects::False));
    let package_json = dep.package_json.as_ref().unwrap();
    assert_eq!(package_json.name(), Some("pkg"));
    assert_eq!(package_json.r#type(), Some("module"));
    assert_eq!(package_json.realpath(), cwd.join("package.json"));
    assert_eq!(
      package_json.check_side_effects_for("styles/app.css"),
      Some(true),
      "sideEffects globs must survive the roundtrip"
    );

    let external = &entry.resolved_deps[ImportRecordIdx::from_raw(1)];
    assert_eq!(external.id.as_str(), "react");
    assert!(external.external.is_external());
    assert!(external.is_external_without_side_effects);
  }

  #[test]
  fn rejects_unknown_version_and_garbage() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = encode_entry("code", &ModuleType::Js, None, &[], &IndexVec::new(), &cwd).unwrap();
    let mut wrong_version = bytes.clone();
    wrong_version[4] = FORMAT_VERSION + 1;
    assert!(decode_entry(&wrong_version, &cwd).is_none());
    assert!(decode_entry(&bytes[0..HEADER_LEN - 1], &cwd).is_none());
    assert!(decode_entry(b"not a cache entry", &cwd).is_none());
  }
}
