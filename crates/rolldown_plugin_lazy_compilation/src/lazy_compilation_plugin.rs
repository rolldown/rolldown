use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::{ImportKind, ModuleId};
use rolldown_plugin::{HookResolveIdOutput, HookUsage, Plugin, PluginContextResolveOptions};
use rolldown_utils::dashmap::FxDashSet;

use crate::runtime_injector::{
  LazyCompilationRuntimeInjector, create_unwrap_lazy_compilation_entry_helper,
};

/// Shared type for lazy entries set
pub type SharedLazyEntries = Arc<FxDashSet<ArcStr>>;

/// Context for lazy compilation, shared between plugin and DevEngine
#[derive(Clone)]
pub struct LazyCompilationContext {
  pub lazy_entries: SharedLazyEntries,
  /// Tracks which proxy modules have been fetched (requested at runtime via `/lazy`)
  pub fetched_entries: SharedLazyEntries,
}

impl LazyCompilationContext {
  /// Check if a module is a lazy module
  pub fn is_lazy_module(&self, module_id: &str) -> bool {
    self.lazy_entries.contains(module_id)
  }

  /// Mark a proxy module as fetched. This changes the content returned by the load hook
  /// from a stub (fetches via /lazy endpoint) to actual code that imports the real module.
  pub fn mark_as_fetched(&self, proxy_module_id: &str) {
    self.fetched_entries.insert(proxy_module_id.into());
  }
}

#[derive(Debug)]
pub struct LazyCompilationPlugin {
  lazy_entries: SharedLazyEntries,
  /// Tracks which proxy modules have been fetched (requested at runtime via `/lazy`)
  fetched_entries: SharedLazyEntries,
  /// The current working directory, obtained from build_start hook
  cwd: OnceLock<PathBuf>,
}

impl LazyCompilationPlugin {
  /// Creates a new LazyCompilationPlugin
  pub fn new() -> Self {
    let lazy_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    let fetched_entries: SharedLazyEntries = Arc::new(FxDashSet::default());
    LazyCompilationPlugin { lazy_entries, fetched_entries, cwd: OnceLock::new() }
  }

  /// Returns a context that can be used to interact with lazy compilation state
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
    HookUsage::BuildStart | HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
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
    if matches!(args.kind, ImportKind::DynamicImport) {
      // If the importer is a fetched proxy module, don't create another proxy.
      // This allows the fetched template's `import($MODULE_ID)` to resolve
      // to the actual module instead of creating a self-referencing proxy.
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
    _ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.contains("rolldown-lazy=1") {
      if self.lazy_entries.contains(args.id) {
        // Extract original ID without the query string (this is the absolute path)
        let original_id = args.id.split("?rolldown-lazy=1").next().unwrap_or(args.id);

        // Compute stable_id from original_id using cwd
        let cwd = self
          .cwd
          .get()
          .ok_or_else(|| anyhow::format_err!("CWD not set in LazyCompilationPlugin"))?;

        let stable_id = ModuleId::new(original_id).stabilize(cwd);

        // Check if this proxy has been fetched (requested at runtime via /lazy)
        // If fetched, return template that imports the real module
        // Otherwise, return stub template that fetches via /lazy endpoint
        let template = if self.fetched_entries.contains(args.id) {
          include_str!("./proxy-module-template-fetched.js")
        } else {
          include_str!("./proxy-module-template.js")
        };

        // The proxy module ID includes the ?rolldown-lazy=1 suffix
        let proxy_id = args.id;

        // Replace placeholders in order: longer ones first to avoid partial matches
        // $PROXY_MODULE_ID and $STABLE_MODULE_ID contain "MODULE_ID" as substring

        // // TODO: hyf0 prevent xss vulnerabilities by escaping IDs properly
        let code = template
          .replace("$PROXY_MODULE_ID", &format!("\"{proxy_id}\""))
          .replace("$STABLE_MODULE_ID", &format!("\"{}\"", stable_id.as_str()))
          .replace("$MODULE_ID", &format!("\"{original_id}\""));
        return Ok(Some(rolldown_plugin::HookLoadOutput {
          code: ArcStr::from(code),
          ..Default::default()
        }));
      }
    }

    Ok(None)
  }

  async fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    // Skip proxy modules (they have their own structure)
    if args.id.contains("?rolldown-lazy=1") {
      return Ok(args.ast);
    }

    args.ast.program.with_mut(|fields| {
      let mut visitor = LazyCompilationRuntimeInjector::new(fields.allocator);
      visitor.visit_program(fields.program);

      // Inject helper after directive prologues (e.g., "use strict")
      if visitor.transformed_count > 0 {
        let helper = create_unwrap_lazy_compilation_entry_helper(fields.allocator);
        // Find insertion point after directive prologues
        let insert_idx = fields
          .program
          .body
          .iter()
          .take_while(|stmt| {
            matches!(stmt, oxc::ast::ast::Statement::ExpressionStatement(expr_stmt)
              if matches!(&expr_stmt.expression, oxc::ast::ast::Expression::StringLiteral(_)))
          })
          .count();
        fields.program.body.insert(insert_idx, helper);
      }
    });

    Ok(args.ast)
  }
}
