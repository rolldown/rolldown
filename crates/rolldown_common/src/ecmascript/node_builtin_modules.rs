use oxc_resolver::NODEJS_BUILTINS;

/// A list of prefix-only modules
const NODEJS_PREFIXED_BUILTINS: &[&str] = &[
  // https://nodejs.org/api/modules.html#built-in-modules-with-mandatory-node-prefix
  "node:sea",
  "node:sqlite",
  "node:test",
  "node:test/reporters",
];

/// Using `phf` should be faster, but it would increase the compile time, since this function is
/// not frequently used, we use `binary_search` instead.
pub fn is_existing_node_builtin_modules(specifier: &str) -> bool {
  if let Some(stripped) = specifier.strip_prefix("node:") {
    return NODEJS_BUILTINS.binary_search(&stripped).is_ok()
      || NODEJS_PREFIXED_BUILTINS.binary_search(&specifier).is_ok();
  }
  NODEJS_BUILTINS.binary_search(&specifier).is_ok()
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
