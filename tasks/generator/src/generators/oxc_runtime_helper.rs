use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

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

    let version = runtime_package
      .package_json()
      .unwrap()
      .raw_json()
      .as_object()
      .and_then(|obj| obj.get("version"))
      .and_then(|v| v.as_str())
      .unwrap_or("unknown");

    let runtime_dir = runtime_package.path().parent().unwrap();
    let esm_helpers_dir = runtime_dir.join("src/helpers/esm");

    // Read all ESM helper files (use BTreeMap for deterministic ordering)
    let mut helpers = BTreeMap::new();
    if esm_helpers_dir.exists() {
      for entry in fs::read_dir(&esm_helpers_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("js") {
          let file_name = path.file_stem().unwrap().to_str().unwrap();
          let content = fs::read_to_string(&path)?;
          helpers.insert(file_name.to_string(), content);
        }
      }
    }

    // Generate the embedded_helpers.rs code
    let code = generate_embedded_helpers_rs(version, &helpers);

    Ok(vec![crate::output::Output::RustString {
      path: output_path("crates/rolldown_plugin_oxc_runtime/src", "embedded_helpers.rs"),
      code: add_header(&code, self.file_path(), "//"),
    }])
  }
}

fn generate_embedded_helpers_rs(version: &str, helpers: &BTreeMap<String, String>) -> String {
  let mut code = String::new();

  // Write file header with version info
  write!(
    &mut code,
    "// This file contains embedded @oxc-project/runtime ESM helpers\n\
     // @oxc-project/runtime version: {version}\n\n"
  )
  .unwrap();

  // Write the static helper map using phf with ArcStr values
  write!(
    &mut code,
    r#"use arcstr::ArcStr;
use phf::{{Map, phf_map}};

pub const RUNTIME_HELPER_PREFIX: &str = "@oxc-project+runtime@{version}/helpers/";
pub const RUNTIME_HELPER_UNVERSIONED_PREFIX: &str = "@oxc-project/runtime/helpers/";

/// Map of all ESM helpers from @oxc-project/runtime/src/helpers/esm/
pub static ESM_HELPERS: Map<&'static str, ArcStr> = phf_map! {{
"#
  )
  .unwrap();

  for (helper_name, content) in helpers {
    // Calculate hash count for raw string literal
    let hash_count = calculate_hash_count(content);
    let hashes = "#".repeat(hash_count);

    writeln!(&mut code, "  \"{helper_name}\" => arcstr::literal!(r{hashes}\"{content}\"{hashes}),")
      .unwrap();
  }

  code.push_str("};\n\n");

  // Write helper functions
  code.push_str(
    r#"/// Get the content of a helper by its specifier
pub fn get_helper_content(specifier: &str) -> Option<ArcStr> {
  let helper_name = specifier.strip_prefix(RUNTIME_HELPER_PREFIX)?;
  ESM_HELPERS.get(helper_name.strip_suffix(".js").unwrap_or(helper_name)).cloned()
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
