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
  assert!(
    result.expect("[Technical Errors]: Failed to bundle.").errors.is_empty(),
    "[Business Errors] Failed to bundle."
  );
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
  assert!(
    result.expect("[Technical Errors]: Failed to bundle.").errors.is_empty(),
    "[Business Errors] Failed to bundle."
  );
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
      .map(|e| (e.kind(), e.into_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
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
      let content = if hidden_runtime_module {
        RUNTIME_MODULE_OUTPUT_RE.replace_all(content, "")
      } else {
        Cow::Borrowed(content)
      };

      [Cow::Owned(format!("## {}\n", asset.filename())), "```js".into(), content, "```".into()]
    })
    .collect::<Vec<_>>()
    .join("\n");
  ret.push_str(&artifacts);

  ret
}

pub(crate) static RUNTIME_MODULE_OUTPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(//#region rolldown:runtime[\s\S]*?//#endregion)")
    .expect("invalid runtime module output regex")
});

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
