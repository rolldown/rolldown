mod utils;
mod utils_2;

use std::{borrow::Cow, path::PathBuf};

use oxc::{
  ast::AstBuilder,
  ast_visit::{Visit, VisitMut},
};
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookTransformAstArgs, HookTransformAstReturn, HookTransformOutput, HookUsage, Plugin,
  PluginContext,
};
use rolldown_plugin_utils::constants::AllocatorPool;
use sugar_path::SugarPath;

#[derive(Debug, Default)]
pub struct ViteImportGlobPluginV2Config {
  pub sourcemap: bool,
}

#[derive(Debug, Default)]
pub struct ViteImportGlobPlugin {
  /// vite also support `source_map` config, but we can't support it now.
  /// Since the source map now follow the codegen option.
  pub root: Option<String>,
  pub restore_query_extension: bool,
  pub is_v2: Option<ViteImportGlobPluginV2Config>,
}

impl Plugin for ViteImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-import-glob")
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
      let allocator_pool = ctx.meta().get_or_insert_default::<AllocatorPool>();
      let allocator_guard = allocator_pool.inner.get();
      let source_type = match args.module_type {
        ModuleType::Js => oxc::span::SourceType::mjs(),
        ModuleType::Jsx => oxc::span::SourceType::jsx(),
        ModuleType::Ts => oxc::span::SourceType::ts(),
        ModuleType::Tsx => oxc::span::SourceType::tsx(),
        _ => unreachable!(),
      };
      let parser_ret = oxc::parser::Parser::new(&allocator_guard, args.code, source_type).parse();
      if parser_ret.panicked
        && let Some(err) =
          parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
      {
        return Err(anyhow::anyhow!(format!(
          "Failed to parse code in '{}': {:?}",
          args.id, err.message
        )));
      }
      let id = args.id.to_slash_lossy();
      let root = self.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(ctx.cwd());
      let mut visitor = utils_2::GlobImportVisit {
        ctx: &ctx,
        root,
        id: &id,
        current: 0,
        code: args.code,
        magic_string: None,
        import_decls: Vec::new(),
        restore_query_extension: self.restore_query_extension,
      };
      visitor.visit_program(&parser_ret.program);
      if let Some(magic_string) = visitor.magic_string {
        return Ok(Some(HookTransformOutput {
          code: Some(magic_string.to_string()),
          map: self.is_v2.as_ref().and_then(|config| {
            config.sourcemap.then(|| {
              magic_string.source_map(string_wizard::SourceMapOptions {
                hires: string_wizard::Hires::Boundary,
                source: args.id.into(),
                ..Default::default()
              })
            })
          }),
          ..Default::default()
        }));
      }
    }
    Ok(None)
  }

  async fn transform_ast(
    &self,
    ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let id = args.id.to_slash_lossy();
      let root = self.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(args.cwd);
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor = utils::GlobImportVisit {
        ctx,
        root,
        id: &id,
        ast_builder,
        current: 0,
        import_decls: ast_builder.vec(),
        restore_query_extension: self.restore_query_extension,
      };
      visitor.visit_program(fields.program);
      if !visitor.import_decls.is_empty() {
        fields.program.body.extend(visitor.import_decls);
      }
    });
    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.is_v2.is_some() { HookUsage::Transform } else { HookUsage::TransformAst }
  }
}
