mod matcher;
mod utils;

use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use arcstr::ArcStr;
use oxc::ast_visit::VisitJs;
use rolldown_common::WatcherChangeKind;
use rolldown_plugin::{
  HookHotUpdateArgs, HookHotUpdateReturn, HookTransformOutput, HookTransformOutputMap, HookUsage,
  Plugin, PluginContext,
};
use rolldown_plugin_utils::parse_program;
use rolldown_utils::dashmap::FxDashMap;
use sugar_path::SugarPath as _;

use crate::matcher::GlobMatcher;

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
  pub root: Option<String>,
  pub sourcemap: bool,
  pub restore_query_extension: bool,
  pub glob_matchers: FxDashMap<ArcStr, Vec<GlobMatcher>>,
}

impl Plugin for ViteImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-import-glob")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::HotUpdate
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.code.contains("import.meta.glob") {
      self.remove_globs(args.id);
      return Ok(None);
    }

    let allocator = oxc::allocator::Allocator::default();
    let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)? else {
      self.remove_globs(args.id);
      return Ok(None);
    };

    let id = args.id.to_slash_lossy();
    let root = self.root.as_ref().map(PathBuf::from);
    let root = root.as_ref().unwrap_or(ctx.cwd());
    let is_dev_mode = ctx.options().is_dev_mode_enabled();
    let mut visitor = utils::GlobImportVisit {
      ctx: &ctx,
      root,
      id: &id,
      current: 0,
      code: args.code,
      magic_string: None,
      import_decls: Vec::new(),
      errors: Vec::new(),
      restore_query_extension: self.restore_query_extension,
      is_dev_mode,
      matchers: Vec::new(),
    };
    visitor.visit_program(&parser_ret.program);
    if let Some(err) = visitor.errors.into_iter().next() {
      return Err(err);
    }
    if is_dev_mode {
      self.set_globs(&id, visitor.matchers);
    }
    if let Some(magic_string) = visitor.magic_string {
      return Ok(Some(HookTransformOutput {
        code: Some(magic_string.to_string()),
        map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
          magic_string.source_map(string_wizard::SourceMapOptions {
            hires: string_wizard::Hires::Boundary,
            source: args.id.into(),
            ..Default::default()
          })
        }),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn hot_update(
    &self,
    _ctx: &PluginContext,
    args: &HookHotUpdateArgs,
  ) -> HookHotUpdateReturn {
    // A content edit cannot change which files a glob matches, so `update` is declined
    if matches!(args.kind, WatcherChangeKind::Update) || self.glob_matchers.is_empty() {
      return Ok(None);
    }

    let mut selected = Vec::new();
    let is_dir = Path::new(args.file.as_str()).is_dir();
    for entry in &self.glob_matchers {
      let id = entry.key();

      // The walk excludes the glob's own module to keep it from importing itself
      if id.as_str() == args.file.as_str() {
        continue;
      }

      let matchers = entry.value();
      let touched = matchers
        .iter()
        .any(|matcher| matcher.matches(&args.file) || matcher.touches_base(&args.file))
        || (is_dir && matchers.iter().any(|matcher| matcher.may_gain_matches_below(&args.file)));

      if touched {
        selected.push(id.clone());
      }
    }

    if selected.is_empty() {
      return Ok(None);
    }

    // Appending rather than replacing, like vite's `[...oldModules, ...modules]`
    let mut modules = Vec::with_capacity(args.modules.len() + selected.len());
    modules.extend_from_slice(&args.modules);
    for id in selected {
      if !modules.contains(&id) {
        modules.push(id);
      }
    }
    Ok(Some(modules))
  }
}
