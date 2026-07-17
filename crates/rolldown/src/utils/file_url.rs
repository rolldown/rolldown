/// Prefixes recognized on `import.meta.<prefix><referenceId>` file URL references.
///
/// `ROLLDOWN_FILE_URL_` is a rolldown-specific alias of Rollup's `ROLLUP_FILE_URL_`.
pub const FILE_URL_PREFIXES: [&str; 2] = ["ROLLDOWN_FILE_URL_", "ROLLUP_FILE_URL_"];

/// Returns the `<referenceId>` when `name` begins with a recognized file URL prefix.
pub fn strip_file_url_prefix(name: &str) -> Option<&str> {
  FILE_URL_PREFIXES.iter().find_map(|prefix| name.strip_prefix(prefix))
}

/// Whether `name` begins with a recognized file URL prefix.
pub fn starts_with_file_url_prefix(name: &str) -> bool {
  strip_file_url_prefix(name).is_some()
}
