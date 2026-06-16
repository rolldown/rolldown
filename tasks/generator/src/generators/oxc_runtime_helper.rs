use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use flate2::{Compression, write::DeflateEncoder};
use oxc_resolver::ResolveOptions;

use crate::{
  define_generator,
  output::{add_header, output_path},
};

use super::{Context, Generator, Runner};

pub struct OxcRuntimeHelperGenerator;

define_generator!(OxcRuntimeHelperGenerator);

impl Generator for OxcRuntimeHelperGenerator {
  fn generate_many(&self, ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    let workspace_root = &ctx.workspace_root;

    // Use oxc_resolver to find @oxc-project/runtime
    let resolver = oxc_resolver::Resolver::new(ResolveOptions::default());
    let runtime_package = resolver.resolve(workspace_root, "@oxc-project/runtime/package.json")?;

    let version = runtime_package.package_json().and_then(|v| v.version()).unwrap_or("unknown");

    let runtime_dir = runtime_package.path().parent().unwrap();
    let esm_helpers_dir = runtime_dir.join("src/helpers/esm");
    let cjs_helpers_dir = runtime_dir.join("src/helpers");

    // Use BTreeMap for deterministic ordering.
    let esm_helpers = read_helpers_dir(&esm_helpers_dir)?;
    // The CJS helpers live alongside the `esm/` subdirectory; `read_helpers_dir` filters
    // non-files so the `esm/` subdir entry is ignored when listing this directory.
    let cjs_helpers = read_helpers_dir(&cjs_helpers_dir)?;

    let code = generate_embedded_helpers_rs(version, &esm_helpers, &cjs_helpers);

    Ok(vec![crate::output::Output::RustString {
      path: output_path("crates/rolldown_plugin_oxc_runtime/src", "embedded_helpers.rs"),
      code: add_header(&code, self.file_path(), "//"),
    }])
  }
}

fn read_helpers_dir(dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
  let mut helpers = BTreeMap::new();
  if !dir.exists() {
    return Ok(helpers);
  }
  for entry in fs::read_dir(dir)? {
    let path = entry?.path();
    if !path.is_file() {
      continue;
    }
    if path.extension().and_then(|s| s.to_str()) != Some("js") {
      continue;
    }
    let file_name = path.file_stem().unwrap().to_str().unwrap();
    let content = fs::read_to_string(&path)?;
    helpers.insert(file_name.to_string(), content);
  }
  Ok(helpers)
}

fn generate_embedded_helpers_rs(
  version: &str,
  esm_helpers: &BTreeMap<String, String>,
  cjs_helpers: &BTreeMap<String, String>,
) -> String {
  let mut code = String::new();

  // Write file header with version info
  write!(
    &mut code,
    "// This file contains embedded @oxc-project/runtime helpers (both ESM and CJS variants).\n\
     // @oxc-project/runtime version: {version}\n\n"
  )
  .unwrap();

  write!(
    &mut code,
    r#"use std::io::Read as _;
use std::sync::{{OnceLock, RwLock}};

use arcstr::ArcStr;
use phf::{{Map, phf_map}};
use rustc_hash::FxHashMap;

pub const RUNTIME_HELPER_PREFIX: &str = "@oxc-project+runtime@{version}/helpers/";
pub const RUNTIME_HELPER_UNVERSIONED_PREFIX: &str = "@oxc-project/runtime/helpers/";

/// A single embedded helper.
///
/// The JS body is stored as raw-DEFLATE-compressed bytes (no zlib/gzip header) to keep the
/// shipped binary small. The `flate2` inflate code is already linked into the binary (via
/// `oxc_compat -> oxc-browserslist -> flate2`), so decompressing here adds essentially no new
/// code.
pub struct Helper {{
  /// Raw-DEFLATE-compressed JS body.
  compressed: &'static [u8],
  /// Length in bytes of the inflated JS body (used to pre-size the buffer).
  len: usize,
}}

impl Helper {{
  /// Inflate this helper's JS body into an `ArcStr`.
  fn inflate(&self) -> ArcStr {{
    let mut decoder = flate2::read::DeflateDecoder::new(self.compressed);
    let mut content = String::with_capacity(self.len);
    decoder.read_to_string(&mut content).expect("embedded helper must inflate to valid UTF-8");
    ArcStr::from(content)
  }}
}}

/// Cache of already-inflated helper bodies, keyed by the helper's `'static` address (which is
/// unique per entry across both `ESM_HELPERS` and `CJS_HELPERS`). Each helper is inflated at
/// most once per process; subsequent loads return the cached `ArcStr` clone (a cheap refcount
/// bump).
static HELPER_CACHE: OnceLock<RwLock<FxHashMap<usize, ArcStr>>> = OnceLock::new();

/// Inflate a helper, caching the result so repeated lookups are cheap.
fn inflate_cached(helper: &'static Helper) -> ArcStr {{
  let key = std::ptr::from_ref(helper) as usize;
  let cache = HELPER_CACHE.get_or_init(|| RwLock::new(FxHashMap::default()));
  if let Some(cached) = cache.read().unwrap().get(&key) {{
    return cached.clone();
  }}
  let content = helper.inflate();
  cache.write().unwrap().entry(key).or_insert_with(|| content.clone());
  content
}}

"#
  )
  .unwrap();

  write_helper_map(&mut code, "ESM_HELPERS", "src/helpers/esm/", esm_helpers);
  write_helper_map(&mut code, "CJS_HELPERS", "src/helpers/", cjs_helpers);

  // Write helper functions
  code.push_str(
    r#"/// Get the content of a helper by its virtual specifier (with the `\0` prefix already stripped).
///
/// Virtual IDs follow the layout of the upstream `@oxc-project/runtime` package:
///   - `<prefix>esm/<name>.js` -> ESM variant
///   - `<prefix><name>.js`     -> CJS variant
pub fn get_helper_content(specifier: &str) -> Option<ArcStr> {
  let helper_path = specifier.strip_prefix(RUNTIME_HELPER_PREFIX)?;
  let helper_path = helper_path.strip_suffix(".js").unwrap_or(helper_path);
  if let Some(name) = helper_path.strip_prefix("esm/") {
    ESM_HELPERS.get(name).map(inflate_cached)
  } else {
    CJS_HELPERS.get(helper_path).map(inflate_cached)
  }
}

/// Check if a specifier is an OXC runtime helper
pub fn is_runtime_helper(specifier: &str) -> bool {
  specifier.starts_with(RUNTIME_HELPER_UNVERSIONED_PREFIX)
}

/// Check if a specifier is a virtual runtime helper (with \0 prefix)
pub fn is_virtual_runtime_helper(specifier: &str) -> bool {
  specifier.starts_with(RUNTIME_HELPER_PREFIX)
}
"#,
  );

  code
}

fn write_helper_map(
  code: &mut String,
  map_name: &str,
  source_dir_doc: &str,
  helpers: &BTreeMap<String, String>,
) {
  writeln!(
    code,
    "/// Map of all helpers from `@oxc-project/runtime/{source_dir_doc}`.\n\
     ///\n\
     /// Values are the raw-DEFLATE-compressed JS bodies, inflated lazily on first use.\n\
     pub static {map_name}: Map<&'static str, Helper> = phf_map! {{"
  )
  .unwrap();

  for (helper_name, content) in helpers {
    let compressed = deflate_raw(content.as_bytes());
    let bytes = format_byte_string(&compressed);
    writeln!(
      code,
      "  \"{helper_name}\" => Helper {{ compressed: {bytes}, len: {} }},",
      content.len()
    )
    .unwrap();
  }

  code.push_str("};\n\n");
}

/// Compress `data` with raw DEFLATE (no zlib/gzip header) at maximum level.
fn deflate_raw(data: &[u8]) -> Vec<u8> {
  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
  encoder.write_all(data).unwrap();
  encoder.finish().unwrap()
}

/// Format `bytes` as a Rust byte-string literal, e.g. `b"\x00\xff..."`.
fn format_byte_string(bytes: &[u8]) -> String {
  let mut out = String::with_capacity(bytes.len() * 4 + 3);
  out.push_str("b\"");
  for &byte in bytes {
    match byte {
      b'\\' => out.push_str("\\\\"),
      b'"' => out.push_str("\\\""),
      // Keep printable ASCII (excluding `\` and `"` handled above) readable.
      0x20..=0x7e => out.push(byte as char),
      _ => write!(out, "\\x{byte:02x}").unwrap(),
    }
  }
  out.push('"');
  out
}
