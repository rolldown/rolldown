use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{
  ImportAttributes, ImportKind, LAZY_PROXY_QUERY, is_lazy_proxy, lazy_proxy_target,
};
use rolldown_plugin::{
  CustomField, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
  PluginContextResolveOptions,
};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

pub type SharedLazyEntries = Arc<FxDashSet<ArcStr>>;

#[derive(Clone)]
pub struct LazyCompilationContext {
  pub fetched_entries: SharedLazyEntries,
}

impl LazyCompilationContext {
  /// Returns whether the id was not marked before.
  pub fn mark_as_fetched(&self, proxy_module_id: &str) -> bool {
    self.fetched_entries.insert(proxy_module_id.into())
  }

  /// Undoes `mark_as_fetched` after a failed scan. Otherwise a later full build would load the
  /// fetched template and pull in a broken route that no page loaded.
  pub fn unmark_fetched(&self, proxy_module_id: &str) {
    self.fetched_entries.remove(proxy_module_id);
  }
}

/// The `resolveId` call of a dynamic import that a lazy proxy replaced.
#[derive(Debug, Clone)]
struct OriginalResolveCall {
  specifier: ArcStr,
  importer: Option<ArcStr>,
  importer_raw_id: Option<ArcStr>,
  kind: ImportKind,
  attributes: ImportAttributes,
  custom: Arc<CustomField>,
}

#[derive(Debug)]
pub struct LazyCompilationPlugin {
  lazy_entries: SharedLazyEntries,
  fetched_entries: SharedLazyEntries,
  /// Keyed by proxy id. Each resolution of the proxy replaces the call. Like `lazy_entries`, it is
  /// never cleaned up.
  original_calls: FxDashMap<ArcStr, OriginalResolveCall>,
}

impl LazyCompilationPlugin {
  pub fn new() -> Self {
    let lazy_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    let fetched_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    LazyCompilationPlugin { lazy_entries, fetched_entries, original_calls: FxDashMap::default() }
  }

  pub fn context(&self) -> LazyCompilationContext {
    LazyCompilationContext { fetched_entries: Arc::clone(&self.fetched_entries) }
  }
}

impl Plugin for LazyCompilationPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "lazy-compilation".into()
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    // Unknown proxy ids must fall through and stay unresolvable — that is what stops an
    // arbitrary id from being bundled (cf. `HmrStage::render_patch`).
    if is_lazy_proxy(args.specifier) && self.lazy_entries.contains(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: None,
        normalize_external_id: None,
        side_effects: None,
        package_json_path: None,
        attributes: None,
        module_options_receipt: None,
      }));
    }

    // Lazy compilation does not support import attributes yet, so such an import is bundled as
    // usual. Without `experimental.importAttributes`, hooks see no attributes, and the import is
    // compiled lazily like any other.
    if matches!(args.kind, ImportKind::DynamicImport) && args.attributes.is_empty() {
      // Without this the fetched template's `import($MODULE_ID)` would be given the proxy's
      // own id back, so the proxy would import itself and the real module never enter the graph.
      if let Some(importer) = args.importer {
        if is_lazy_proxy(importer) && self.fetched_entries.contains(importer) {
          return self.resolve_with_original_call(ctx, importer, args.specifier).await;
        }
      }

      let original_id = ctx
        .resolve(
          args.specifier,
          args.importer,
          Some(PluginContextResolveOptions {
            import_kind: ImportKind::DynamicImport,
            is_entry: false,
            skip_self: true,
            custom: std::sync::Arc::<rolldown_plugin::CustomField>::clone(&args.custom),
            attributes: args.attributes.clone(),
          }),
        )
        .await??;

      // An external has no factory in the dev runtime, so `requestLazy` cannot run it.
      // Return `None` rather than this result: `HookResolveIdOutput` would drop fields
      // such as `is_external_without_side_effects`.
      // See internal-docs/lazy-compilation/implementation.md.
      if original_id.external.is_external() {
        return Ok(None);
      }

      // `ctx.resolve` can re-enter this hook, so the id may already carry the marker. Appending
      // it twice would yield an id nothing else in the graph agrees on.
      let lazy_id: ArcStr = if is_lazy_proxy(original_id.id.as_str()) {
        original_id.id.as_str().into()
      } else {
        format!("{}{LAZY_PROXY_QUERY}", original_id.id).into()
      };
      self.lazy_entries.insert(lazy_id.clone());
      // The latest call replaces an earlier one, so the fetch follows the importer as it is now.
      self.original_calls.insert(
        lazy_id.clone(),
        OriginalResolveCall {
          specifier: args.specifier.into(),
          importer: args.importer.map(Into::into),
          importer_raw_id: args.importer_raw_id.map(Into::into),
          kind: args.kind,
          attributes: args.attributes.clone(),
          custom: Arc::clone(&args.custom),
        },
      );

      return Ok(Some(HookResolveIdOutput {
        id: lazy_id,
        external: None,
        normalize_external_id: None,
        side_effects: None,
        package_json_path: None,
        attributes: None,
        // The proxy is not the module that `original_id` points to, so it does not start with
        // the module options of that resolution.
        module_options_receipt: None,
      }));
    }

    Ok(None)
  }

  async fn load(
    &self,
    _ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    let Some(module_id) = lazy_proxy_target(args.id) else { return Ok(None) };
    if !self.lazy_entries.contains(args.id) {
      return Ok(None);
    }

    let code = if self.fetched_entries.contains(args.id) {
      render_proxy_template(include_str!("./proxy-module-template-fetched.js"), module_id)?
    } else {
      include_str!("./proxy-module-template.js").to_string()
    };
    Ok(Some(rolldown_plugin::HookLoadOutput { code: ArcStr::from(code), ..Default::default() }))
  }
}

impl LazyCompilationPlugin {
  /// The fetched proxy `proxy_id` imports its real module `real_id`. Resolves the real module
  /// again with the call that the proxy replaced, so that call, not the proxy's import, creates
  /// the real module, with its `meta` and `moduleSideEffects`, as without lazy compilation.
  /// Falls back to `None` (the next plugins resolve `real_id` with the proxy as importer) when
  /// no call is stored, its importer file is gone, or the call fails or now gives another module.
  /// See internal-docs/lazy-compilation/implementation.md.
  async fn resolve_with_original_call(
    &self,
    ctx: &PluginContext,
    proxy_id: &str,
    real_id: &str,
  ) -> HookResolveIdReturn {
    let Some(call) = self.original_calls.get(proxy_id).map(|call| call.clone()) else {
      return Ok(None);
    };
    if let Some(importer) = call.importer_raw_id.as_deref() {
      let path = Path::new(importer.split_once('?').map_or(importer, |(path, _)| path));
      if path.is_absolute() && !path.exists() {
        return Ok(None);
      }
    }
    let Ok(Ok(resolved)) = ctx
      .resolve(
        &call.specifier,
        call.importer.as_deref(),
        Some(PluginContextResolveOptions {
          import_kind: call.kind,
          is_entry: false,
          skip_self: true,
          custom: Arc::clone(&call.custom),
          attributes: call.attributes.clone(),
        }),
      )
      .await
    else {
      return Ok(None);
    };
    if resolved.external.is_external() || resolved.id.as_str() != real_id {
      return Ok(None);
    }
    Ok(Some(HookResolveIdOutput::from_resolved_id(resolved)))
  }
}

/// Quotes the id as a JSON string, so Windows backslashes stay escaped (#9102).
fn render_proxy_template(template: &str, module_id: &str) -> serde_json::Result<String> {
  Ok(template.replace("$MODULE_ID", &serde_json::to_string(module_id)?))
}

#[cfg(test)]
mod tests {
  use super::render_proxy_template;

  #[test]
  fn windows_path() {
    let rendered =
      render_proxy_template("import($MODULE_ID);", r"D:\Users\foo\bar\baz.js").unwrap();
    assert_eq!(rendered, r#"import("D:\\Users\\foo\\bar\\baz.js");"#);
  }

  #[test]
  fn unix_path() {
    let rendered = render_proxy_template("import($MODULE_ID);", "/Users/foo/bar.js").unwrap();
    assert_eq!(rendered, r#"import("/Users/foo/bar.js");"#);
  }
}
