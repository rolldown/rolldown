use oxc_resolver::NODEJS_BUILTINS;

/// A list of prefix-only modules
const NODEJS_PREFIXED_BUILTINS: &[&str] = &[
  // https://nodejs.org/api/modules.html#built-in-modules-with-mandatory-node-prefix
  "node:sea",
  "node:sqlite",
  "node:test",
  "node:test/reporters",
];

/// While `phf` would offer faster lookups, it increases compile time.
/// Since this function is not performance-critical, we prefer `contains`,
/// which is faster than `binary_search` in this case — see <https://github.com/oxc-project/oxc/issues/10076> for details.
pub fn is_existing_node_builtin_modules(specifier: &str) -> bool {
  if let Some(stripped) = specifier.strip_prefix("node:") {
    return NODEJS_BUILTINS.contains(&stripped) || NODEJS_PREFIXED_BUILTINS.contains(&specifier);
  }
  NODEJS_BUILTINS.contains(&specifier)
}

#[test]
fn test_is_builtin_modules() {
  // not prefix-only modules
  assert!(is_existing_node_builtin_modules("fs"));
  assert!(is_existing_node_builtin_modules("node:fs"));
  // prefix-only modules
  assert!(is_existing_node_builtin_modules("node:test"));
  // not a builtin module
  assert!(!is_existing_node_builtin_modules("unknown"));
  assert!(!is_existing_node_builtin_modules("node:unknown"));
}
