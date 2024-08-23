use std::path::{Path, PathBuf};

use rolldown_utils::pattern_filter;
use sugar_path::SugarPath;

use crate::types::hook_filter::HookFilterOptions;

/// If the transform hook is filtered out and need to be skipped.
/// Using `Option<bool>` for better programming experience.
/// return `None` means it is early return, should not be skipped.
/// return `Some(false)` means it should be skipped.
/// return `Some(true)` means it should not be skipped.
pub fn filter_resolve_id(options: &HookFilterOptions, id: &str, cwd: &PathBuf) -> Option<bool> {
  let resolve_id_hook_filter_options = options.resolve_id.as_ref()?;
  let id_filter = resolve_id_hook_filter_options.id.as_ref()?;

  let stabilized_path = Path::new(id).relative(cwd);
  let normalized_id = stabilized_path.to_string_lossy();
  Some(pattern_filter::filter(
    id_filter.exclude.as_ref().map(|item| item.as_slice()),
    id_filter.include.as_ref().map(|item| item.as_slice()),
    id,
    &normalized_id,
  ))
}
