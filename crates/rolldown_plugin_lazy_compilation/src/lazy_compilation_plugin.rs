use arcstr::ArcStr;
use rolldown_common::ImportKind;
use rolldown_plugin::{HookResolveIdOutput, HookUsage, Plugin, PluginContextResolveOptions};
use rolldown_utils::dashmap::FxDashMap;

#[derive(Debug)]
pub struct LazyCompilationPlugin {
  lazy_entries: FxDashMap<ArcStr, ()>,
}

impl LazyCompilationPlugin {
  pub fn new() -> Self {
    LazyCompilationPlugin { lazy_entries: FxDashMap::default() }
  }
}

impl Plugin for LazyCompilationPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    // TODO: hyf0 As more features are implemented in internal plugins, we may want to give them more specific names.
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
      self.lazy_entries.insert(lazy_id.clone(), ());

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
    if args.id.contains("rolldown-lazy=1") && self.lazy_entries.contains_key(args.id) {
      let code = "export {}".to_string();
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: ArcStr::from(code),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}
