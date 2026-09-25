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

    let (code, compressed_helpers) =
      generate_embedded_helpers_rs(version, &esm_helpers, &cjs_helpers)?;

    Ok(vec![
      crate::output::Output::RustString {
        path: output_path("crates/rolldown_plugin_oxc_runtime/src", "embedded_helpers.rs"),
        code: add_header(&code, self.file_path(), "//"),
      },
      crate::output::Output::Binary {
        path: output_path("crates/rolldown_plugin_oxc_runtime/src", "embedded_helpers.deflate"),
        content: compressed_helpers,
      },
    ])
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
) -> anyhow::Result<(String, Vec<u8>)> {
  let mut corpus = String::new();
  let mut helper_metadata = Vec::with_capacity(esm_helpers.len() + cjs_helpers.len());

  for (prefix, helpers) in [("esm/", esm_helpers), ("", cjs_helpers)] {
    for (name, content) in helpers {
      let start = u32::try_from(corpus.len()).expect("Oxc runtime helper corpus fits in u32");
      corpus.push_str(content);
      let end = u32::try_from(corpus.len()).expect("Oxc runtime helper corpus fits in u32");
      helper_metadata.push((format!("{prefix}{name}"), start, end));
    }
  }

  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
  encoder.write_all(corpus.as_bytes())?;
  let compressed_helpers = encoder.finish()?;

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
    r#"use std::{{io::Read as _, sync::LazyLock}};

use arcstr::ArcStr;
use flate2::read::DeflateDecoder;
use phf::{{Map, phf_map}};

pub const RUNTIME_HELPER_PREFIX: &str = "@oxc-project+runtime@{version}/helpers/";
pub const RUNTIME_HELPER_UNVERSIONED_PREFIX: &str = "@oxc-project/runtime/helpers/";

"#
  )
  .unwrap();

  writeln!(&mut code, "const UNCOMPRESSED_HELPERS_LEN: usize = {};", corpus.len()).unwrap();
  code.push_str(
    "static COMPRESSED_HELPERS: &[u8] = include_bytes!(\"embedded_helpers.deflate\");\n\n",
  );

  code.push_str(
    r"static HELPER_SLOTS: Map<&'static str, u16> = phf_map! {
",
  );
  for (slot, (path, _, _)) in helper_metadata.iter().enumerate() {
    let slot = u16::try_from(slot).expect("Oxc runtime helper count fits in u16");
    writeln!(&mut code, "  \"{path}\" => {slot},").unwrap();
  }
  code.push_str("};\n\nstatic HELPER_RANGES: &[(u32, u32)] = &[\n");
  for (_, start, end) in helper_metadata {
    writeln!(&mut code, "  ({start}, {end}),").unwrap();
  }
  code.push_str(
    r#"];

static DECODED_HELPERS: LazyLock<Box<[ArcStr]>> = LazyLock::new(|| {
  let mut decoder = DeflateDecoder::new(COMPRESSED_HELPERS);
  let mut decoded = Vec::with_capacity(UNCOMPRESSED_HELPERS_LEN);
  decoder.read_to_end(&mut decoded).expect("embedded Oxc runtime helpers should decompress");
  assert_eq!(decoded.len(), UNCOMPRESSED_HELPERS_LEN, "embedded Oxc runtime helper length");
  let decoded = String::from_utf8(decoded).expect("embedded Oxc runtime helpers should be UTF-8");

  HELPER_RANGES
    .iter()
    .map(|&(start, end)| ArcStr::from(&decoded[start as usize..end as usize]))
    .collect()
});

"#,
  );

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
  let slot = HELPER_SLOTS.get(helper_path)?;
  Some(DECODED_HELPERS[*slot as usize].clone())
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

  Ok((code, compressed_helpers))
}
