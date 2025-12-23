pub mod snapshot;

use std::{borrow::Cow, path::Path, sync::LazyLock};

use regex::Regex;
use sugar_path::SugarPath;

#[macro_export]
/// `std::file!` alternative that returns an absolute path.
macro_rules! abs_file {
  () => {
    std::path::Path::new(env!("WORKSPACE_DIR")).join(file!())
  };
}

/// Sugar macro for `abs_file!().parent().unwrap()`
#[macro_export]
macro_rules! abs_file_dir {
  () => {
    std::path::Path::new(env!("WORKSPACE_DIR")).join(file!()).parent().unwrap().to_path_buf()
  };
}

pub(crate) static RUNTIME_MODULE_OUTPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(//#region rolldown:runtime[\s\S]*?//#endregion)")
    .expect("invalid runtime module output regex")
});

pub(crate) static HMR_RUNTIME_MODULE_OUTPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(//#region rolldown:hmr[\s\S]*?//#endregion)")
    .expect("invalid hmr runtime module output regex")
});

pub(crate) static OXC_RUNTIME_MODULE_OUTPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(//#region \\0@oxc-project\+runtime@[\s\S]+?//#endregion)")
    .expect("invalid oxc runtime module output regex")
});

// Some content of snapshot are meaningless, we'd like to remove them to reduce the noise when reviewing snapshots.
pub fn tweak_snapshot<'a>(
  content: &'a str,
  hide_runtime_module: bool,
  hide_hmr_runtime: bool,
  cwd: &Path,
) -> Cow<'a, str> {
  let cwd_str = cwd.to_str().unwrap_or("");
  let cwd_slash_str = cwd.to_slash().unwrap();
  let needs_cwd_replacement =
    !cwd_str.is_empty() && (content.contains(cwd_str) || content.contains(cwd_slash_str.as_ref()));

  if !hide_runtime_module
    && !hide_hmr_runtime
    && !content.contains("\\0@oxc-project+runtime@")
    && !needs_cwd_replacement
  {
    return Cow::Borrowed(content);
  }

  let mut result = content.to_string();

  // Replace cwd paths with $CWD to make snapshots portable
  if !cwd_str.is_empty() {
    result = result.replace(cwd_str, "$CWD");
    result = result.replace(&*cwd_slash_str, "$CWD");
  }

  if hide_runtime_module {
    result =
      RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "// HIDDEN [rolldown:runtime]").into_owned();
  }

  if hide_hmr_runtime {
    result =
      HMR_RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "// HIDDEN [rolldown:hmr]").into_owned();
  }

  result = OXC_RUNTIME_MODULE_OUTPUT_RE
    .replace_all(&result, "// HIDDEN [\\0@oxc-project+runtime@0.0.0/file.js]")
    .into_owned();

  Cow::Owned(result)
}
