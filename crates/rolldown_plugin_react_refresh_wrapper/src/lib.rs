use std::{borrow::Cow, sync::LazyLock};

use regex::Regex;
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};

const PLUGIN_NAME: &str = "@vitejs/plugin-react-oxc";
const RUNTIME_PUBLIC_PATH: &str = "/@react-refresh";

static DEFAULT_INCLUDE_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("\\.[tj]sx?(?:$|\\?)").unwrap());
static DEFAULT_EXCLUDE_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("/node_modules/").unwrap());

static REACT_COMP_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("extends\\s+(?:React\\.)?(?:Pure)?Component").unwrap());
const REFRESH_CONTENT: &str = "$RefreshReg$(";

#[derive(Debug)]
pub struct ReactRefreshWrapperPlugin {
  jsx_import_runtime: String,
  jsx_import_dev_runtime: String,
  react_refresh_host: String,
}

impl ReactRefreshWrapperPlugin {
  pub fn new() -> Self {
    let jsx_import_source = "react";
    Self {
      jsx_import_dev_runtime: format!("{jsx_import_source}/jsx-dev-runtime"),
      jsx_import_runtime: format!("{jsx_import_source}/jsx-runtime"),
      react_refresh_host: String::new(),
    }
  }

  fn add_refresh_wrapper(&self, code: &str, id: &str) -> Option<String> {
    let has_refresh = code.contains(REFRESH_CONTENT);
    let only_react_comp = !has_refresh && REACT_COMP_RE.is_match(code);
    if !has_refresh && !only_react_comp {
      return None;
    }

    // TODO: escape other characters
    let escaped_id = format!("\"{id}\"");

    let mut new_code = code.to_string();
    if has_refresh {
      let refresh_head = format!(
        "\
let prevRefreshReg;
let prevRefreshSig;

if (import.meta.hot && !inWebWorker) {{
  if (!window.$RefreshReg$) {{
    throw new Error(
      \"{PLUGIN_NAME} can't detect preamble. Something is wrong.\"
    );
  }}

  prevRefreshReg = window.$RefreshReg$;
  prevRefreshSig = window.$RefreshSig$;
  window.$RefreshReg$ = RefreshRuntime.getRefreshReg({escaped_id});
  window.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
}}
",
      )
      .replace('\n', "");

      new_code = refresh_head
        + &new_code
        + "
if (import.meta.hot && !inWebWorker) {
  window.$RefreshReg$ = prevRefreshReg;
  window.$RefreshSig$ = prevRefreshSig;
}";
    }

    let shared_head = format!(
      "\
import * as RefreshRuntime from \"{}{RUNTIME_PUBLIC_PATH}\";
const inWebWorker = typeof WorkerGlobalScope !== 'undefined' && self instanceof WorkerGlobalScope;
",
      self.react_refresh_host
    )
    .replace('\n', "");

    new_code = shared_head + &new_code + &format!("

if (import.meta.hot && !inWebWorker) {{
  RefreshRuntime.__hmr_import(import.meta.url).then((currentExports) => {{
    RefreshRuntime.registerExportsForReactRefresh({escaped_id}, currentExports);
    import.meta.hot.accept((nextExports) => {{
      if (!nextExports) return;
      const invalidateMessage = RefreshRuntime.validateRefreshBoundaryAndEnqueueUpdate({escaped_id}, currentExports, nextExports);
      if (invalidateMessage) import.meta.hot.invalidate(invalidateMessage);
    }});
  }});
}}
");

    Some(new_code)
  }
}

impl Plugin for ReactRefreshWrapperPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    Cow::Borrowed("builtin:react-refresh-wrapper")
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !DEFAULT_INCLUDE_RE.is_match(args.id) || DEFAULT_EXCLUDE_RE.is_match(args.id) {
      return Ok(None);
    }

    let use_fast_refresh = is_jsx(args.id)
      || args.code.contains(&self.jsx_import_dev_runtime)
      || args.code.contains(&self.jsx_import_runtime);
    if !use_fast_refresh {
      return Ok(None);
    }

    let Some(new_code) = self.add_refresh_wrapper(args.code, args.id) else {
      return Ok(None);
    };
    Ok(Some(HookTransformOutput { code: Some(new_code), map: None, ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

fn is_jsx(id: &str) -> bool {
  let id_without_query = id.split_once('?').map_or(id, |(id, _)| id);
  id_without_query.ends_with('x')
}
