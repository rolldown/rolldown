use oxc_resolver::NODEJS_BUILTINS;

/// Using `phf` should be faster, but it would increase the compile time, since this function is
/// not frequently used, we use `binary_search` instead.
pub fn is_builtin_modules(specifier: &str) -> bool {
  let normalized_specifier =
    if let Some(specifier) = specifier.strip_prefix("node:") { specifier } else { specifier };
  NODEJS_BUILTINS.binary_search(&normalized_specifier).is_ok()
}
