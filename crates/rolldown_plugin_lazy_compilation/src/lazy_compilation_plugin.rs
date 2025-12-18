use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::ImportKind;
use rolldown_plugin::{HookResolveIdOutput, HookUsage, Plugin, PluginContextResolveOptions};
use rolldown_utils::dashmap::FxDashSet;

/// Shared type for lazy entries set
pub type SharedLazyEntries = Arc<FxDashSet<ArcStr>>;

/// Context for lazy compilation, shared between plugin and DevEngine
#[derive(Clone)]
pub struct LazyCompilationContext {
  pub lazy_entries: SharedLazyEntries,
  /// Tracks which proxy modules have been executed (requested at runtime)
  pub executed_entries: SharedLazyEntries,
}

impl LazyCompilationContext {
  /// Check if a module is a lazy module
  pub fn is_lazy_module(&self, module_id: &str) -> bool {
    self.lazy_entries.contains(module_id)
  }

  /// Mark a proxy module as executed. This changes the content returned by the load hook
  /// from a stub (fetches via /lazy endpoint) to actual code that imports the real module.
  pub fn mark_as_executed(&self, proxy_module_id: &str) {
    self.executed_entries.insert(proxy_module_id.into());
  }
}

#[derive(Debug)]
pub struct LazyCompilationPlugin {
  lazy_entries: SharedLazyEntries,
  /// Tracks which proxy modules have been executed (requested at runtime)
  executed_entries: SharedLazyEntries,
}

impl LazyCompilationPlugin {
  /// Creates a new LazyCompilationPlugin
  pub fn new() -> Self {
    let lazy_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    let executed_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    LazyCompilationPlugin { lazy_entries, executed_entries }
  }

  /// Returns a context that can be used to interact with lazy compilation state
  pub fn context(&self) -> LazyCompilationContext {
    LazyCompilationContext {
      lazy_entries: Arc::clone(&self.lazy_entries),
      executed_entries: Arc::clone(&self.executed_entries),
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
    if matches!(args.kind, ImportKind::DynamicImport) {
      // If the importer is an executed proxy module, don't create another proxy.
      // This allows the executed template's `import($MODULE_ID)` to resolve
      // to the actual module instead of creating a self-referencing proxy.
      if let Some(importer) = args.importer {
        if importer.contains("?rolldown-lazy=1") && self.executed_entries.contains(importer) {
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

      let lazy_id: ArcStr = format!("{}?rolldown-lazy=1", original_id.id).into();
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
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.contains("rolldown-lazy=1") {
      if self.lazy_entries.contains(args.id) {
        // Extract original ID without the query string (this is the absolute path)
        let original_id = args.id.split("?rolldown-lazy=1").next().unwrap_or(args.id);

        // Check if this proxy has been executed (requested at runtime)
        // If executed, return template that imports the real module
        // Otherwise, return stub template that fetches via /lazy endpoint
        let template = if self.executed_entries.contains(args.id) {
          include_str!("./proxy-module-template-executed.js")
        } else {
          include_str!("./proxy-module-template.js")
        };

        // The proxy module ID includes the ?rolldown-lazy=1 suffix
        let proxy_id = args.id;

        let code = template
          .replace("$PROXY_MODULE_ID", &format!("\"{proxy_id}\""))
          .replace("$MODULE_ID", &format!("\"{original_id}\""));
        return Ok(Some(rolldown_plugin::HookLoadOutput {
          code: ArcStr::from(code),
          ..Default::default()
        }));
      }
    }

    Ok(None)
  }
}
