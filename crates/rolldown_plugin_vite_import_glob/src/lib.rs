mod matcher;
mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use arcstr::ArcStr;
use oxc::ast::ast::Program;
use oxc::ast_visit::VisitJs;
use rolldown_common::WatcherChangeKind;
use rolldown_plugin::{
  HookHotUpdateArgs, HookHotUpdateReturn, HookTransformArgs, HookTransformOutput,
  HookTransformOutputMap, HookUsage, Plugin, PluginContext, SharedTransformPluginContext,
};
use rolldown_plugin_utils::{
  constants::{ViteImportGlob, ViteImportGlobValue},
  parse_program,
};
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

impl ViteImportGlobPlugin {
  fn transform_program(
    &self,
    ctx: &SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
    id: &str,
    root: &PathBuf,
    program: &Program<'_>,
    resolved_glob_groups: Vec<Vec<(String, Option<String>)>>,
  ) -> anyhow::Result<(Option<HookTransformOutput>, Vec<GlobMatcher>)> {
    let mut visitor = utils::GlobImportVisit {
      ctx,
      root,
      id,
      current: 0,
      code: args.code,
      magic_string: None,
      import_decls: Vec::new(),
      errors: Vec::new(),
      restore_query_extension: self.restore_query_extension,
      resolved_glob_groups: resolved_glob_groups.into(),
      is_dev_mode: ctx.options().is_dev_mode_enabled(),
      matchers: Vec::new(),
    };
    visitor.visit_program(program);
    if let Some(err) = visitor.errors.into_iter().next() {
      return Err(err);
    }
    let output = visitor.magic_string.map(|magic_string| HookTransformOutput {
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
    Ok((output, visitor.matchers))
  }
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
    let is_dev_mode = ctx.options().is_dev_mode_enabled();
    let (output, matchers) = 'glob: {
      if !args.code.contains("import.meta.glob") {
        break 'glob (None, Vec::new());
      }
      let id = args.id.to_slash_lossy().into_owned();
      let configured_root = self.root.as_ref().map(PathBuf::from);
      let root = configured_root.as_ref().unwrap_or(ctx.cwd());
      let glob_groups = {
        let allocator = oxc::allocator::Allocator::default();
        let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)?
        else {
          break 'glob (None, Vec::new());
        };
        let mut resolve_visitor = utils::GlobResolveVisit::default();
        resolve_visitor.visit_program(&parser_ret.program);
        if resolve_visitor.glob_groups.iter().all(Vec::is_empty) {
          break 'glob self.transform_program(
            &ctx,
            args,
            &id,
            root,
            &parser_ret.program,
            resolve_visitor
              .glob_groups
              .into_iter()
              .map(|group| group.into_iter().map(|glob| (glob, None)).collect())
              .collect(),
          )?;
        }
        resolve_visitor.glob_groups
      };

      let mut resolved_glob_groups = Vec::with_capacity(glob_groups.len());
      for glob_group in glob_groups {
        let mut resolved_group = Vec::with_capacity(glob_group.len());
        for glob in glob_group {
          let is_sub_imports_pattern = glob.starts_with('#') && glob.contains('*');
          let mut custom = rolldown_plugin::CustomField::new();
          custom.insert(ViteImportGlob, ViteImportGlobValue(is_sub_imports_pattern));
          let resolved = ctx
            .resolve(
              &glob,
              Some(&id),
              Some(rolldown_plugin::PluginContextResolveOptions {
                custom: Arc::new(custom),
                ..Default::default()
              }),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .map(|resolved| {
              path_posix::normalize(&rolldown_utils::pattern_filter::normalize_path(
                resolved.id.as_str(),
              ))
              .into_owned()
            });
          resolved_group.push((glob, resolved));
        }
        resolved_glob_groups.push(resolved_group);
      }

      // The first AST borrows its allocator, so it cannot live across the
      // await above. Reparse once resolutions are ready.
      let allocator = oxc::allocator::Allocator::default();
      let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)?
      else {
        break 'glob (None, Vec::new());
      };
      self.transform_program(&ctx, args, &id, root, &parser_ret.program, resolved_glob_groups)?
    };
    if is_dev_mode {
      self.set_globs(&args.id.to_slash_lossy(), matchers);
    }
    Ok(output)
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
