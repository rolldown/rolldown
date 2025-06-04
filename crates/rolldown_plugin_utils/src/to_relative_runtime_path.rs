use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils as _;
use sugar_path::SugarPath as _;

const CURRENT_SCRIPT_URL_OR_BASE_URI: &str = "typeof document === 'undefined' ? location.href : document.currentScript && document.currentScript.tagName.toUpperCase() === 'SCRIPT' && document.currentScript.src || document.baseURI";

pub fn create_to_import_meta_url_based_relative_runtime(
  format: &str,
  is_worker: bool,
) -> impl Fn(&Path, &Path) -> String {
  let format = if is_worker && format == "iife" { "worker-iife" } else { format };
  let to_relative_path = match format {
    "cjs" => cjs,
    "es" => es,
    "iife" => iife,
    "umd" => umd,
    "worker-iife" => worker_iife,
    _ => unreachable!("Invalid format: {}", format),
  };
  move |filename: &Path, importer: &Path| -> String {
    let path = filename.relative(importer.parent().unwrap_or(importer));
    to_relative_path(&path.to_slash_lossy())
  }
}

fn cjs(path: &str) -> String {
  format!(
    "(typeof document === 'undefined' ? {} : {})",
    get_file_url_from_relative_path(path),
    get_relative_url_from_document(path, false)
  )
}

fn es(path: &str) -> String {
  format!("new URL('{}', import.meta.url).href", escape_id(&partial_encode_url_path(path)))
}

fn iife(path: &str) -> String {
  get_relative_url_from_document(path, false)
}

fn umd(path: &str) -> String {
  format!(
    "(typeof document === 'undefined' && typeof location === 'undefined' ? {} : {})",
    get_file_url_from_relative_path(path),
    get_relative_url_from_document(path, true)
  )
}

fn worker_iife(path: &str) -> String {
  format!("new URL('{}', self.location.href).href", escape_id(&partial_encode_url_path(path)))
}

fn partial_encode_url_path(url: &str) -> Cow<'_, str> {
  if url.starts_with("data:") {
    return Cow::Borrowed(url);
  }
  let file_path = rolldown_utils::url::clean_url(url);
  Cow::Owned(format!("{}{}", file_path.cow_replace('%', "%25"), &url[file_path.len()..]))
}

fn get_file_url_from_relative_path(path: &str) -> String {
  format!("require('u' + 'rl').pathToFileURL(__dirname + '/{}').href", escape_id(path))
}

fn get_relative_url_from_document(path: &str, is_umd: bool) -> String {
  format!(
    "new URL({}, {}).href",
    escape_id(&partial_encode_url_path(path)),
    if is_umd { CURRENT_SCRIPT_URL_OR_BASE_URI } else { &CURRENT_SCRIPT_URL_OR_BASE_URI[50..] }
  )
}

fn escape_id(id: &str) -> Cow<'_, str> {
  if id.contains(['\n', '\r', '\\', '\u{2028}', '\u{2029}']) {
    let mut result = String::with_capacity(id.len() + 2);
    for c in id.chars() {
      match c {
        '\\' => result.push_str("\\\\"),
        '\n' => result.push_str("\\\n"),
        '\r' => result.push_str("\\\r"),
        '\u{2028}' => result.push_str("\\\u{2028}"),
        '\u{2029}' => result.push_str("\\\u{2029}"),
        _ => result.push(c),
      }
    }
    Cow::Owned(result)
  } else {
    Cow::Borrowed(id)
  }
}
