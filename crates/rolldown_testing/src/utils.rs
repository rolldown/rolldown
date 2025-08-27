use std::{borrow::Cow, sync::LazyLock};

use regex::Regex;
use rolldown_common::BundlerOptions;

pub fn assert_bundled(options: BundlerOptions) {
  let result = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
      let mut bundler = rolldown::Bundler::new(options);
      bundler.generate().await
    });
  assert!(result.is_ok(), "Failed to bundle.");
}

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

// Some content of snapshot are meaningless, we'd like to remove them to reduce the noise when reviewing snapshots.
pub fn tweak_snapshot(
  content: &str,
  hide_runtime_module: bool,
  hide_hmr_runtime: bool,
) -> Cow<'_, str> {
  if !hide_runtime_module && !hide_hmr_runtime {
    return Cow::Borrowed(content);
  }

  let mut result = content.to_string();

  if hide_runtime_module {
    result =
      RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "// HIDDEN [rolldown:runtime]").into_owned();
  }

  if hide_hmr_runtime {
    result =
      HMR_RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "// HIDDEN [rolldown:hmr]").into_owned();
  }

  Cow::Owned(result)
}
