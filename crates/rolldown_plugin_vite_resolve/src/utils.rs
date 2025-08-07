use std::borrow::Cow;

use cow_utils::CowUtils;

pub const BROWSER_EXTERNAL_ID: &str = "__vite-browser-external";
pub const OPTIONAL_PEER_DEP_ID: &str = "__vite-optional-peer-dep";

// bareImportRE.test(id)
pub fn is_bare_import(id: &str) -> bool {
  if is_windows_drive_path(id) {
    return false;
  }

  id.starts_with(|c| is_regex_w_character_class(c) || c == '@') && !id.contains("://")
}

// check for deep import, e.g. "my-lib/foo"
// deepImportRE.test(id)
pub fn is_deep_import(id: &str) -> bool {
  if id.starts_with('@') {
    let split: Vec<&str> = id.splitn(3, '/').collect();
    split.len() == 3 && split[0].len() >= 2 && !split[1].is_empty()
  } else {
    id[1..].contains('/')
  }
}

pub fn get_extension(id: &str) -> &str {
  id.rsplit_once('.').map_or("", |(_, ext)| ext)
}

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Character_class_escape#w
fn is_regex_w_character_class(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

pub fn is_windows_drive_path(id: &str) -> bool {
  let id_bytes = id.as_bytes();
  id_bytes.len() >= 2 && id_bytes[0].is_ascii_alphabetic() && id_bytes[1] == b':'
}

pub fn normalize_path(path: &str) -> Cow<'_, str> {
  // this function does not do normalization by `path.posix.normalize`
  // but for this plugin, it is fine as we only handle paths that are absolute
  path.cow_replace('\\', "/")
}

pub fn get_npm_package_name(id: &str) -> Option<&str> {
  if id.starts_with('@') {
    let mut indices = id.match_indices('/');
    indices.next()?;
    let second_pos = indices.next().map_or(id.len(), |(pos, _)| pos);
    Some(&id[0..second_pos])
  } else {
    id.split('/').next()
  }
}

pub fn can_externalize_file(file_path: &str) -> bool {
  let ext = get_extension(file_path);
  ext.is_empty() || ext == "js" || ext == "mjs" || ext == "cjs"
}

pub fn is_in_node_modules(id: &str) -> bool {
  id.contains("node_modules")
}

/// path.resolve normalizes the leading slashes to a single slash
pub fn normalize_leading_slashes(specifier: &str) -> &str {
  let trimmed = specifier.trim_start_matches('/');
  let leading_slashes = specifier.len() - trimmed.len();
  if leading_slashes <= 1 { specifier } else { &specifier[leading_slashes - 1..] }
}
