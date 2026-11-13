mod matcher;
mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use arcstr::ArcStr;
use oxc::ast_visit::VisitJs;
use rolldown_common::{ModuleInfo, NormalModule, WatcherChangeKind};
use rolldown_plugin::{
  HookFilterArgs, HookHotUpdateArgs, HookHotUpdateReturn, HookNoopReturn, HookTransformOutput,
  HookTransformOutputMap, HookUsage, Plugin, PluginContext, PluginHookMeta,
};
use rolldown_plugin_utils::parse_program;
use rolldown_utils::{
  dashmap::{FxDashMap, FxDashSet},
  indexmap::FxIndexMap,
};
use sugar_path::SugarPath as _;

use crate::matcher::GlobMatcher;

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
  pub root: Option<String>,
  pub sourcemap: bool,
  pub restore_query_extension: bool,
  pub glob_matchers: FxDashMap<ArcStr, Vec<GlobMatcher>>,
  /// In dev, the modules whose `transform` ran in their current module task.
  pub transformed: FxDashSet<ArcStr>,
}

impl Plugin for ViteImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-import-glob")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::Transform | HookUsage::ModuleParsed | HookUsage::HotUpdate
  }

  // The expanded glob depends on which files exist in a directory, and Rolldown cannot validate
  // a directory after a restart.
  fn transform_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: Some(false) })
  }

  // A module without a glob still gets a saved result. When it reuses that result, `transform`
  // does not run, and `module_parsed` removes its old matchers.
  fn hook_filter_matches(&self, _ctx: &PluginContext, args: HookFilterArgs<'_>) -> bool {
    match args {
      HookFilterArgs::Transform(args) => args.code.contains("import.meta.glob"),
      _ => true,
    }
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    // A module task that failed after `transform` leaves its mark.
    self.transformed.clear();
    Ok(())
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let is_dev_mode = ctx.options().is_dev_mode_enabled();
    let mut matchers = Vec::new();
    let mut output = None;
    if args.code.contains("import.meta.glob") {
      let id = args.id.to_slash_lossy();
      let root = self.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(ctx.cwd());
      let mut visitor = utils::GlobImportVisit {
        ctx: &ctx,
        root,
        id: &id,
        code: args.code,
        magic_string: None,
        import_decls: Vec::new(),
        errors: Vec::new(),
        restore_query_extension: self.restore_query_extension,
        is_dev_mode,
        matchers: Vec::new(),
        glob_calls: Vec::new(),
        resolved_globs: FxIndexMap::default(),
      };
      {
        let allocator = oxc::allocator::Allocator::default();
        if let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)? {
          visitor.visit_program(&parser_ret.program);
        }
      }
      visitor.resolve_globs().await;
      visitor.transform();
      if let Some(err) = visitor.errors.into_iter().next() {
        return Err(err);
      }
      matchers = visitor.matchers;
      output = visitor.magic_string.map(|magic_string| HookTransformOutput {
        code: Some(magic_string.to_string()),
        map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
          magic_string.source_map(string_wizard::SourceMapOptions {
            hires: string_wizard::Hires::Boundary,
            source: args.id.into(),
            ..Default::default()
          })
        }),
        ..Default::default()
      });
    }
    if is_dev_mode {
      let id = args.id.to_slash_lossy();
      self.set_globs(&id, matchers);
      self.transformed.insert(ArcStr::from(id.as_ref()));
    }
    Ok(output)
  }

  // In dev, a full scan in the same process can reuse a saved result. `transform` then does not
  // run, and the module may still have matchers from a version that had a glob. A saved result
  // never has a glob, because `transform` has the cache off.
  async fn module_parsed(
    &self,
    ctx: &PluginContext,
    _module_info: Arc<ModuleInfo>,
    normal_module: &NormalModule,
  ) -> HookNoopReturn {
    if ctx.options().is_dev_mode_enabled() {
      let id = normal_module.id.as_str().to_slash_lossy();
      if self.transformed.remove(id.as_ref()).is_none() {
        self.glob_matchers.remove(id.as_ref());
      }
    }
    Ok(())
  }

  async fn hot_update(
    &self,
    _ctx: &PluginContext,
    args: &HookHotUpdateArgs,
  ) -> HookHotUpdateReturn {
    if args.kind == WatcherChangeKind::Update {
      return Ok(None);
    }

    let mut owners = self
      .glob_matchers
      .iter()
      .filter(|entry| entry.value().iter().any(|matcher| matcher.is_match(&args.file)))
      .map(|entry| entry.key().clone())
      .collect::<Vec<_>>();
    if owners.is_empty() {
      return Ok(None);
    }
    // The map has no stable order.
    owners.sort_unstable();

    Ok(Some(args.modules.iter().cloned().chain(owners).collect()))
  }
}
