use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

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
    r#"use arcstr::ArcStr;
use phf::{{Map, phf_map}};

pub const RUNTIME_HELPER_PREFIX: &str = "@oxc-project+runtime@{version}/helpers/";
pub const RUNTIME_HELPER_UNVERSIONED_PREFIX: &str = "@oxc-project/runtime/helpers/";

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
    ESM_HELPERS.get(name).cloned()
  } else {
    CJS_HELPERS.get(helper_path).cloned()
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
     pub static {map_name}: Map<&'static str, ArcStr> = phf_map! {{"
  )
  .unwrap();

  for (helper_name, content) in helpers {
    let hash_count = calculate_hash_count(content);
    let hashes = "#".repeat(hash_count);
    writeln!(code, "  \"{helper_name}\" => arcstr::literal!(r{hashes}\"{content}\"{hashes}),")
      .unwrap();
  }

  code.push_str("};\n\n");
}

/// Calculate the number of # needed for raw string literal
fn calculate_hash_count(content: &str) -> usize {
  let mut count = 0;
  let mut chars = content.chars().peekable();

  while let Some(ch) = chars.next() {
    if ch == '"' {
      let mut hash_seq = 0;
      while chars.peek() == Some(&'#') {
        chars.next();
        hash_seq += 1;
      }
      count = count.max(hash_seq);
    }
  }

  count + 1 // Add one more to be safe
}
