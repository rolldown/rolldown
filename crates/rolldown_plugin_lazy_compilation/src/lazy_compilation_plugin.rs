use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use arcstr::ArcStr;
use rolldown_common::{ImportKind, ModuleId};
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
  cwd: OnceLock<PathBuf>,
}

impl LazyCompilationPlugin {
  pub fn new() -> Self {
    let lazy_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    let fetched_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    LazyCompilationPlugin { lazy_entries, fetched_entries, cwd: OnceLock::new() }
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
    HookUsage::BuildStart | HookUsage::ResolveId | HookUsage::Load
  }

  async fn build_start(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let _ = self.cwd.set(args.options.cwd.clone());
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    // Unknown proxy ids must fall through and stay unresolvable — that is what stops an
    // arbitrary id from being bundled (cf. `HmrStage::compile_lazy_entry`).
    if args.specifier.ends_with("?rolldown-lazy=1") && self.lazy_entries.contains(args.specifier) {
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
        if importer.contains("?rolldown-lazy=1") && self.fetched_entries.contains(importer) {
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
      let lazy_id: ArcStr = if original_id.id.as_str().ends_with("?rolldown-lazy=1") {
        original_id.id.as_str().into()
      } else {
        format!("{}?rolldown-lazy=1", original_id.id).into()
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
    if args.id.contains("rolldown-lazy=1") {
      if self.lazy_entries.contains(args.id) {
        let original_id = args.id.split("?rolldown-lazy=1").next().unwrap_or(args.id);
        let cwd = self
          .cwd
          .get()
          .ok_or_else(|| anyhow::format_err!("CWD not set in LazyCompilationPlugin"))?;

        let stable_id = ModuleId::new(original_id).stabilize(cwd);

        let template = if self.fetched_entries.contains(args.id) {
          include_str!("./proxy-module-template-fetched.js")
        } else {
          include_str!("./proxy-module-template.js")
        };

        let proxy_id = args.id;

        let stable_proxy_id = format!("{stable_id}?rolldown-lazy=1");

        let code =
          render_proxy_template(template, proxy_id, &stable_id, &stable_proxy_id, original_id)?;
        return Ok(Some(rolldown_plugin::HookLoadOutput {
          code: ArcStr::from(code),
          ..Default::default()
        }));
      }
    }

    Ok(None)
  }
}

// Replace placeholders in order: longer ones first to avoid partial matches
fn render_proxy_template(
  template: &str,
  proxy_id: &str,
  stable_id: &str,
  stable_proxy_id: &str,
  original_id: &str,
) -> serde_json::Result<String> {
  Ok(
    template
      .replace("$PROXY_MODULE_ID", &serde_json::to_string(proxy_id)?)
      .replace("$STABLE_MODULE_ID", &serde_json::to_string(stable_id)?)
      .replace("$STABLE_PROXY_MODULE_ID", &serde_json::to_string(stable_proxy_id)?)
      .replace("$MODULE_ID", &serde_json::to_string(original_id)?),
  )
}

#[cfg(test)]
mod tests {
  use super::render_proxy_template;

  #[test]
  fn windows_path() {
    let proxy_id = r"D:\Users\foo\bar\baz.js?rolldown-lazy=1";
    let stable_id = r"src\bar\baz.js";
    let stable_proxy_id = r"src\bar\baz.js?rolldown-lazy=1";
    let original_id = r"D:\Users\foo\bar\baz.js";

    let template = "P=$PROXY_MODULE_ID;S=$STABLE_MODULE_ID;M=$MODULE_ID;";
    let rendered =
      render_proxy_template(template, proxy_id, stable_id, stable_proxy_id, original_id).unwrap();

    assert_eq!(
      rendered,
      r#"P="D:\\Users\\foo\\bar\\baz.js?rolldown-lazy=1";S="src\\bar\\baz.js";M="D:\\Users\\foo\\bar\\baz.js";"#
    );
  }

  #[test]
  fn unix_path() {
    let id = "/Users/foo/bar.js?rolldown-lazy=1";
    let rendered = render_proxy_template(
      "$PROXY_MODULE_ID",
      id,
      "src/bar.js",
      "src/bar.js?rolldown-lazy=1",
      "/Users/foo/bar.js",
    )
    .unwrap();
    assert_eq!(rendered, "\"/Users/foo/bar.js?rolldown-lazy=1\"");
  }
}
