mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use oxc::ast::ast::Program;
use oxc::ast_visit::Visit;
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
  Plugin, SharedTransformPluginContext,
};
use rolldown_plugin_utils::constants::{ViteImportGlob, ViteImportGlobValue};
use sugar_path::SugarPath as _;

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
  pub root: Option<String>,
  pub sourcemap: bool,
  pub restore_query_extension: bool,
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
  ) -> HookTransformReturn {
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
    };
    visitor.visit_program(program);
    if let Some(err) = visitor.errors.into_iter().next() {
      return Err(err);
    }
    Ok(visitor.magic_string.map(|magic_string| HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
        magic_string.source_map(string_wizard::SourceMapOptions {
          hires: string_wizard::Hires::Boundary,
          source: args.id.into(),
          ..Default::default()
        })
      }),
      ..Default::default()
    }))
  }
}

impl Plugin for ViteImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-import-glob")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if matches!(
      args.module_type,
      ModuleType::Js | ModuleType::Ts | ModuleType::Jsx | ModuleType::Tsx
    ) && args.code.contains("import.meta.glob")
    {
      let source_type = match args.module_type {
        ModuleType::Js => oxc::span::SourceType::mjs(),
        ModuleType::Jsx => oxc::span::SourceType::jsx(),
        ModuleType::Ts => oxc::span::SourceType::ts(),
        ModuleType::Tsx => oxc::span::SourceType::tsx(),
        _ => unreachable!(),
      };
      let id = args.id.to_slash_lossy().into_owned();
      let configured_root = self.root.as_ref().map(PathBuf::from);
      let root = configured_root.as_ref().unwrap_or(ctx.cwd());
      let glob_groups = {
        let allocator = oxc::allocator::Allocator::default();
        let parser_ret = oxc::parser::Parser::new(&allocator, args.code, source_type)
          .with_options(oxc::parser::ParseOptions {
            preserve_parens: false,
            ..oxc::parser::ParseOptions::default()
          })
          .parse();
        if parser_ret.panicked
          && let Some(err) =
            parser_ret.diagnostics.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
        {
          return Err(anyhow::anyhow!(format!(
            "Failed to parse code in '{}': {:?}",
            args.id, err.message
          )));
        }
        let mut resolve_visitor = utils::GlobResolveVisit::default();
        resolve_visitor.visit_program(&parser_ret.program);
        if resolve_visitor.glob_groups.iter().all(Vec::is_empty) {
          return self.transform_program(
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
          );
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

      // The first AST cannot live across the await above because it borrows
      // its allocator. Reparse once resolutions are ready, then transform
      // synchronously without blocking the CurrentThread runtime.
      let allocator = oxc::allocator::Allocator::default();
      let parser_ret = oxc::parser::Parser::new(&allocator, args.code, source_type)
        .with_options(oxc::parser::ParseOptions {
          preserve_parens: false,
          ..oxc::parser::ParseOptions::default()
        })
        .parse();
      return self.transform_program(
        &ctx,
        args,
        &id,
        root,
        &parser_ret.program,
        resolved_glob_groups,
      );
    }
    Ok(None)
  }
}
