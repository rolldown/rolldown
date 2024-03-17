use regex::Regex;
use std::path::Path;

pub fn strip_extended_prefix<P: AsRef<Path>>(path: P) -> Option<String> {
  let path = path.as_ref();
  let prefix = r"\\?\";

  if cfg!(target_os = "windows") {
    let path_str = path.to_str()?;
    if path_str.starts_with(prefix) {
      Some(path_str.strip_prefix(prefix)?.replace("\\", "/"))
    } else {
      Some(path_str.replace("\\", "/"))
    }
  } else {
    path.to_str().map(|s| s.to_string())
  }
}

pub fn normalize_error_windows_path(s: String) -> String {
  if cfg!(target_os = "windows") {
    let re = Regex::new(r"\[.*?\]").unwrap();
    if re.is_match(&s) {
      re.replace_all(&s, |caps: &regex::Captures| caps[0].replace("\\", "/")).to_string()
    } else {
      s
    }
  } else {
    s
  }
}
