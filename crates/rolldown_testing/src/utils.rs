use std::{borrow::Cow, path::Path, sync::LazyLock};

use regex::Regex;
use rolldown::BundleOutput;
use rolldown_common::{BundlerOptions, Output};
use rolldown_error::DiagnosticOptions;

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

pub fn assert_bundled_write(options: BundlerOptions) {
  let result = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .expect("Failed building the Runtime")
    .block_on(async move {
      let mut bundler = rolldown::Bundler::new(options);
      bundler.write().await
    });
  assert!(result.is_ok(), "Failed to bundle.");
}

pub fn stringify_bundle_output(output: BundleOutput, cwd: &Path) -> String {
  let hidden_runtime_module = true;

  let mut ret = String::new();
  let mut assets = output.assets;
  // Make the snapshot consistent
  let mut warnings = output.warnings;
  warnings.sort_by(|a, b| {
    let a = a.to_string();
    let b = b.to_string();
    a.cmp(&b)
  });
  if !warnings.is_empty() {
    ret.push_str("# warnings\n\n");
    let diagnostics = warnings
      .into_iter()
      .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
    let rendered = diagnostics
      .flat_map(|(code, diagnostic)| {
        [
          Cow::Owned(format!("## {code}\n")),
          "```text".into(),
          Cow::Owned(diagnostic.to_string()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n");
    ret.push_str(&rendered);
    ret.push('\n');
  }

  ret.push_str("# Assets\n\n");
  assets.sort_by_key(|c| c.filename().to_string());
  let artifacts = assets
    .iter()
    .filter(|asset| !asset.filename().contains("$runtime$") && matches!(asset, Output::Chunk(_)))
    .flat_map(|asset| {
      let content = std::str::from_utf8(asset.content_as_bytes()).unwrap();
      let content = tweak_snapshot(content, hidden_runtime_module, true);

      [Cow::Owned(format!("## {}\n", asset.filename())), "```js".into(), content, "```".into()]
    })
    .collect::<Vec<_>>()
    .join("\n");
  ret.push_str(&artifacts);

  ret
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

// Match pattern like `@oxc-project+runtime@0.77.0`
pub(crate) static OXC_PROJECT_RUNTIME_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(@oxc-project\+runtime@\d+\.\d+\.\d+)")
    .expect("invalid hmr runtime module output regex")
});

// Some content of snapshot are meaningless, we'd like to remove them to reduce the noise when reviewing snapshots.
pub fn tweak_snapshot(
  content: &str,
  hide_runtime_module: bool,
  hide_hmr_runtime: bool,
) -> Cow<str> {
  if !hide_runtime_module && !hide_hmr_runtime {
    return Cow::Borrowed(content);
  }

  let mut result = content.to_string();

  if hide_runtime_module {
    result = RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "").into_owned();
  }

  if hide_hmr_runtime {
    result =
      HMR_RUNTIME_MODULE_OUTPUT_RE.replace_all(&result, "// HIDDEN [rolldown:hmr]").into_owned();
  }

  // Replace pattern `@oxc-project+runtime@0.77.0` with `@oxc-project+runtime@VERSION` to avoid unnecessary changes in bumping version.
  result = OXC_PROJECT_RUNTIME_RE.replace_all(&result, "@oxc-project+runtime@VERSION").into_owned();

  Cow::Owned(result)
}
