use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{ImportKind, LAZY_PROXY_QUERY, is_lazy_proxy, lazy_proxy_target};
use rolldown_plugin::{HookResolveIdOutput, HookUsage, Plugin, PluginContextResolveOptions};
use rolldown_utils::dashmap::FxDashSet;

pub type SharedLazyEntries = Arc<FxDashSet<ArcStr>>;

#[derive(Clone)]
pub struct LazyCompilationContext {
  pub lazy_entries: SharedLazyEntries,
  pub fetched_entries: SharedLazyEntries,
}

impl LazyCompilationContext {
  pub fn mark_as_fetched(&self, proxy_module_id: &str) {
    self.fetched_entries.insert(proxy_module_id.into());
  }
}

#[derive(Debug)]
pub struct LazyCompilationPlugin {
  lazy_entries: SharedLazyEntries,
  fetched_entries: SharedLazyEntries,
}

impl LazyCompilationPlugin {
  pub fn new() -> Self {
    let lazy_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    let fetched_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    LazyCompilationPlugin { lazy_entries, fetched_entries }
  }

  pub fn context(&self) -> LazyCompilationContext {
    LazyCompilationContext {
      lazy_entries: Arc::clone(&self.lazy_entries),
      fetched_entries: Arc::clone(&self.fetched_entries),
    }
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
    // arbitrary id from being bundled (cf. `HmrStage::compile_lazy_entry`).
    if is_lazy_proxy(args.specifier) && self.lazy_entries.contains(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: None,
        normalize_external_id: None,
        side_effects: None,
        package_json_path: None,
      }));
    }

    if matches!(args.kind, ImportKind::DynamicImport) {
      // Without this the fetched template's `import($MODULE_ID)` would be given the proxy's
      // own id back, so the proxy would import itself and the real module never enter the graph.
      if let Some(importer) = args.importer {
        if is_lazy_proxy(importer) && self.fetched_entries.contains(importer) {
          return Ok(None);
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
          }),
        )
        .await??;

      // `ctx.resolve` can re-enter this hook, so the id may already carry the marker. Appending
      // it twice would yield an id nothing else in the graph agrees on.
      let lazy_id: ArcStr = if is_lazy_proxy(original_id.id.as_str()) {
        original_id.id.as_str().into()
      } else {
        format!("{}{LAZY_PROXY_QUERY}", original_id.id).into()
      };
      self.lazy_entries.insert(lazy_id.clone());

      return Ok(Some(HookResolveIdOutput {
        id: lazy_id,
        external: None,
        normalize_external_id: None,
        side_effects: None,
        package_json_path: None,
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
