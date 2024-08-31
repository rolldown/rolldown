use std::path::{Path, PathBuf};

use rolldown_common::ModuleType;
use rolldown_utils::pattern_filter::{self, FilterResult};
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
  Some(
    pattern_filter::filter(
      id_filter.exclude.as_deref(),
      id_filter.include.as_deref(),
      id,
      &normalized_id,
    )
    .inner(),
  )
}

pub fn filter_load(options: &HookFilterOptions, id: &str, cwd: &PathBuf) -> Option<bool> {
  let load_hook_filter_options = options.load.as_ref()?;
  let id_filter = load_hook_filter_options.id.as_ref()?;

  let stabilized_path = Path::new(id).relative(cwd);
  let normalized_id = stabilized_path.to_string_lossy();
  Some(
    pattern_filter::filter(
      id_filter.exclude.as_deref(),
      id_filter.include.as_deref(),
      id,
      &normalized_id,
    )
    .inner(),
  )
}

/// Since transform has three different filter, so we need to check all of them.
pub fn filter_transform(
  options: &HookFilterOptions,
  id: &str,
  cwd: &PathBuf,
  module_type: &ModuleType,
  code: &str,
) -> bool {
  let Some(transform_hook_filter_options) = options.transform.as_ref() else {
    return true;
  };

  let mut fallback_ret =
    if let Some(ref module_type_filter) = transform_hook_filter_options.module_type {
      if module_type_filter.iter().any(|ty| ty == module_type) {
        return true;
      }
      false
    } else {
      true
    };
  if let Some(ref id_filter) = transform_hook_filter_options.id {
    let stabilized_path = Path::new(id).relative(cwd);
    let normalized_id = stabilized_path.to_string_lossy();
    let id_res = pattern_filter::filter(
      id_filter.exclude.as_deref(),
      id_filter.include.as_deref(),
      id,
      &normalized_id,
    );
    // it matched by `exclude` or `include`, early return
    if let FilterResult::Match(id_res) = id_res {
      return id_res;
    }
    fallback_ret = fallback_ret && id_res.inner();
  }

  if let Some(ref code_filter) = transform_hook_filter_options.code {
    let code_res = pattern_filter::filter_code(
      code_filter.exclude.as_deref(),
      code_filter.include.as_deref(),
      code,
    );
    // it matched by `exclude` or `include`, early return
    if let FilterResult::Match(code_res) = code_res {
      return code_res;
    }
    fallback_ret = fallback_ret && code_res.inner();
  }
  fallback_ret
}
