use std::{ffi::OsString, path::Path};

use cow_utils::CowUtils as _;
use rolldown_common::ModuleId;
use rolldown_utils::pattern_filter::normalize_path;

pub fn get_chunk_original_name(
  root: &Path,
  is_legacy: bool,
  chunk_name: &str,
  facade_module_id: Option<&ModuleId>,
) -> Option<String> {
  facade_module_id.map(|module_id| {
    let mut name = module_id.relative_path(root);
    if is_legacy && !chunk_name.contains("-legacy") {
      let extension = OsString::from(name.extension().unwrap_or_default());
      if let Some(stem) = name.file_stem() {
        let mut file_stem = OsString::with_capacity(stem.len() + 7);
        file_stem.push(stem);
        file_stem.push("-legacy");
        name.set_file_name(file_stem);
      }
      name.set_extension(extension);
    }
    let name = name.to_string_lossy();
    let name = normalize_path(&name);
    name.cow_replace('\0', "").into_owned()
  })
}
