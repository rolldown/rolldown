pub mod snapshot;

use std::{borrow::Cow, sync::LazyLock};

use regex::Regex;

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
  Regex::new(r"(//#region \\0rolldown/runtime\.js[\s\S]*?//#endregion)")
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

/// A column-zero declaration and the name it binds, for probing what a runtime region span
/// actually declares.
static TOP_LEVEL_DECL_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"(?m)^(?:var|let|const|class)\s+([A-Za-z_$][\w$]*)|^(?:async\s+)?function\s*\*?\s*([A-Za-z_$][\w$]*)",
  )
  .expect("invalid top-level declaration probe regex")
});

/// Top-level names the dev runtime glue adds to the `\0rolldown/runtime.js` region, from
/// `rolldown_plugin_hmr/src/runtime/runtime-extra-dev-*.js` and the test harness's
/// `hmr-runtime.js`. If a name is added there, dev-mode snapshots will show the runtime region
/// raw until it is added here too.
const DEV_RUNTIME_TOP_LEVEL_NAMES: &[&str] = &[
  "Module",
  "MissingFactoryError",
  "DevRuntime",
  "BaseDevRuntime",
  "ModuleHotContext",
  "DefaultDevRuntime",
  "loadScript",
  "clientId",
  "addr",
  "socket",
  "TestHotContext",
  "TestDevRuntime",
];

/// Whether a matched runtime region span really contains only runtime code.
///
/// Region markers are ordinary comments, and the default `minify: 'dce-only'` pass drops the
/// comments attached to a statement it removes. A removed statement at a module boundary takes
/// the adjacent `//#endregion` + `//#region` pair with it, after which the surviving markers
/// still pair up textually — the lazy match then extends across the next module and hiding it
/// would silently swallow user code. Marker structure cannot reveal this; content can: the
/// runtime region only ever declares `__`-prefixed helpers plus the known dev-runtime names, so
/// any other column-zero declaration proves foreign code, and the span is left visible instead —
/// a noisy-but-truthful snapshot beats one that hides user code. (Indented regions inside
/// iife/umd wrappers have no column-zero declarations and are hidden as before; the corruption
/// this guards against arises from esm strict-order layouts.)
fn runtime_region_is_hidable(span: &str) -> bool {
  span.lines().skip(1).all(|line| !line.trim_start().starts_with("//#region "))
    && TOP_LEVEL_DECL_NAME_RE.captures_iter(span).all(|caps| {
      caps.get(1).or_else(|| caps.get(2)).is_none_or(|name| {
        name.as_str().starts_with("__") || DEV_RUNTIME_TOP_LEVEL_NAMES.contains(&name.as_str())
      })
    })
}

// Some content of snapshot are meaningless, we'd like to remove them to reduce the noise when reviewing snapshots.
pub fn tweak_snapshot(
  content: &str,
  hide_runtime_module: bool,
  hide_hmr_runtime: bool,
) -> Cow<'_, str> {
  if !hide_runtime_module && !hide_hmr_runtime && !content.contains("\\0@oxc-project+runtime@") {
    return Cow::Borrowed(content);
  }

  let mut result = content.to_string();

  if hide_runtime_module {
    result = RUNTIME_MODULE_OUTPUT_RE
      .replace_all(&result, |caps: &regex::Captures| {
        if runtime_region_is_hidable(&caps[0]) {
          "// HIDDEN [\\0rolldown/runtime.js]".to_string()
        } else {
          caps[0].to_string()
        }
      })
      .into_owned();
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

#[cfg(test)]
mod tests {
  use super::tweak_snapshot;

  #[test]
  fn hides_a_pure_runtime_region() {
    let content = "//#region \\0rolldown/runtime.js\nvar __esmMin = (fn) => fn;\nfunction __reExport(a) {\n\treturn a;\n}\n//#endregion\n//#region main.js\nconsole.log(1);\n//#endregion\n";
    let tweaked = tweak_snapshot(content, true, false);
    assert!(tweaked.contains("// HIDDEN [\\0rolldown/runtime.js]"));
    assert!(!tweaked.contains("__esmMin"));
    assert!(tweaked.contains("console.log(1);"));
  }

  #[test]
  fn hides_a_runtime_region_with_dev_runtime_declarations() {
    let content = "//#region \\0rolldown/runtime.js\nvar __esmMin = (fn) => fn;\nvar Module = class {};\nvar DevRuntime = class {};\n//#endregion\n";
    let tweaked = tweak_snapshot(content, true, false);
    assert!(tweaked.contains("// HIDDEN [\\0rolldown/runtime.js]"));
    assert!(!tweaked.contains("DevRuntime"));
  }

  #[test]
  fn keeps_a_region_that_swallowed_user_code() {
    // The dce-only pass removed a statement at the runtime/user boundary together with its
    // attached `//#endregion` + `//#region foo.js` pair, so the runtime region now textually
    // extends over `init_foo`. The remaining markers still pair up; only the non-runtime
    // declaration reveals the foreign code.
    let content = "//#region \\0rolldown/runtime.js\nvar __esmMin = (fn) => fn;\nfunction init_foo() {\n\treturn (init_foo = __esmMin(0))();\n}\n//#endregion\n//#region main.js\nconsole.log(1);\n//#endregion\n";
    let tweaked = tweak_snapshot(content, true, false);
    assert!(!tweaked.contains("// HIDDEN"));
    assert!(tweaked.contains("function init_foo()"));
  }

  #[test]
  fn keeps_a_region_that_crossed_into_another_region() {
    // No closer of its own before the next module's region: the lazy span would end at that
    // module's closer, so the contained foreign opener must refuse the hide.
    let content = "//#region \\0rolldown/runtime.js\nvar __esmMin = (fn) => fn;\n//#region foo.js\nvar __state = 1;\n//#endregion\n";
    let tweaked = tweak_snapshot(content, true, false);
    assert!(!tweaked.contains("// HIDDEN"));
    assert!(tweaked.contains("//#region foo.js"));
  }
}
