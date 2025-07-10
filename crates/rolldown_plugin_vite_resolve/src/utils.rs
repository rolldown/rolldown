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

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Character_class_escape#w
fn is_regex_w_character_class(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

pub fn is_windows_drive_path(id: &str) -> bool {
  let id_bytes = id.as_bytes();
  id_bytes.len() >= 2 && id_bytes[0].is_ascii_alphabetic() && id_bytes[1] == b':'
}

pub fn normalize_path(path: &str) -> Cow<str> {
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

/// path.resolve normalizes the leading slashes to a single slash
pub fn normalize_leading_slashes(specifier: &str) -> &str {
  let trimmed = specifier.trim_start_matches('/');
  let leading_slashes = specifier.len() - trimmed.len();
  if leading_slashes <= 1 { specifier } else { &specifier[leading_slashes - 1..] }
}
