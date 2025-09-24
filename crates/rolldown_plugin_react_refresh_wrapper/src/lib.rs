use std::{borrow::Cow, fmt::Write, sync::LazyLock};

use arcstr::ArcStr;
use regex::Regex;
use rolldown_plugin::{
  HookResolveIdOutput, HookTransformOutput, HookUsage, Plugin, PluginHookMeta, PluginOrder,
};
use rolldown_plugin_utils::to_string_literal;
use rolldown_utils::pattern_filter::{FilterResult, StringOrRegex, filter};

const PLUGIN_NAME: &str = "@vitejs/plugin-react";
const RUNTIME_PUBLIC_PATH: ArcStr = arcstr::literal!("/@react-refresh");

static REACT_COMP_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("extends\\s+(?:React\\.)?(?:Pure)?Component").unwrap());
const REFRESH_CONTENT: &str = "$RefreshReg$(";

#[derive(Debug)]
pub struct ReactRefreshWrapperPluginOptions {
  pub cwd: String,
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_import_source: String,
  pub react_refresh_host: String,
}

#[derive(Debug)]
pub struct ReactRefreshWrapperPlugin {
  cwd: String,
  include: Vec<StringOrRegex>,
  exclude: Vec<StringOrRegex>,
  jsx_import_runtime: String,
  jsx_import_dev_runtime: String,
  react_refresh_host: String,
}

impl ReactRefreshWrapperPlugin {
  pub fn new(options: ReactRefreshWrapperPluginOptions) -> Self {
    let jsx_import_source = options.jsx_import_source;
    Self {
      cwd: options.cwd,
      include: options.include,
      exclude: options.exclude,
      jsx_import_dev_runtime: format!("{jsx_import_source}/jsx-dev-runtime"),
      jsx_import_runtime: format!("{jsx_import_source}/jsx-runtime"),
      react_refresh_host: options.react_refresh_host,
    }
  }

  fn add_refresh_wrapper(&self, code: &str, id: &str) -> Option<String> {
    let has_refresh = memchr::memmem::find(code.as_bytes(), REFRESH_CONTENT.as_bytes()).is_some();
    let only_react_comp = !has_refresh && REACT_COMP_RE.is_match(code);
    if !has_refresh && !only_react_comp {
      return None;
    }

    let escaped_id = to_string_literal(id);

    let mut new_code = code.to_string();
    write!(new_code, "\
import * as RefreshRuntime from \"{}{RUNTIME_PUBLIC_PATH}\";
const inWebWorker = typeof WorkerGlobalScope !== 'undefined' && self instanceof WorkerGlobalScope;
import * as __vite_react_currentExports from {escaped_id};
if (import.meta.hot && !inWebWorker) {{
  if (!window.$RefreshReg$) {{
    throw new Error(
      \"{PLUGIN_NAME} can't detect preamble. Something is wrong.\"
    );
  }}

  const currentExports = __vite_react_currentExports;
  RefreshRuntime.registerExportsForReactRefresh({escaped_id}, currentExports);
  import.meta.hot.accept((nextExports) => {{
    if (!nextExports) return;
    const invalidateMessage = RefreshRuntime.validateRefreshBoundaryAndEnqueueUpdate({escaped_id}, currentExports, nextExports);
    if (invalidateMessage) import.meta.hot.invalidate(invalidateMessage);
  }});
}}
",
      self.react_refresh_host).unwrap();

    if has_refresh {
      write!(
        new_code,
        "function $RefreshReg$(type, id) {{ return RefreshRuntime.register(type, {escaped_id} + ' ' + id); }}
function $RefreshSig$() {{ return RefreshRuntime.createSignatureFunctionForTransform(); }}
",
      )
      .unwrap();
    }

    Some(new_code)
  }
}

impl Plugin for ReactRefreshWrapperPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    Cow::Borrowed("builtin:react-refresh-wrapper")
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.specifier == RUNTIME_PUBLIC_PATH {
      return Ok(Some(HookResolveIdOutput { id: RUNTIME_PUBLIC_PATH, ..Default::default() }));
    }
    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if matches!(
      filter(Some(&self.exclude), Some(&self.include), args.id, &self.cwd),
      FilterResult::Match(false) | FilterResult::NoneMatch(false)
    ) {
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
    HookUsage::ResolveId | HookUsage::Transform
  }
}

fn is_jsx(id: &str) -> bool {
  let id_without_query = id.split_once('?').map_or(id, |(id, _)| id);
  id_without_query.ends_with('x')
}
