use std::{borrow::Cow, path::PathBuf};

use oxc::{ast::AstBuilder, ast_visit::VisitMut};
use rolldown_plugin::{
  HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin, PluginContext,
};
use sugar_path::SugarPath;

mod utils;

use utils::GlobImportVisit;

#[derive(Debug, Default)]
pub struct ImportGlobPlugin {
  pub config: ImportGlobPluginConfig,
}

#[derive(Debug, Default)]
pub struct ImportGlobPluginConfig {
  /// vite also support `source_map` config, but we can't support it now.
  /// Since the source map now follow the codegen option.
  pub root: Option<String>,
  pub restore_query_extension: bool,
}

impl Plugin for ImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:import-glob")
  }

  async fn transform_ast(
    &self,
    ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    let mut visitor_errors = Vec::new();

    args.ast.program.with_mut(|fields| {
      let id = args.id.to_slash_lossy();
      let root = self.config.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(args.cwd);

      let ast_builder = AstBuilder::new(fields.allocator);

      let mut visitor = GlobImportVisit {
        ctx,
        root,
        id: &id,
        ast_builder,
        restore_query_extension: self.config.restore_query_extension,
        current: 0,
        import_decls: ast_builder.vec(),
        errors: vec![],
      };
      visitor.visit_program(fields.program);
      visitor_errors.extend(visitor.errors);
      if !visitor.import_decls.is_empty() {
        fields.program.body.extend(visitor.import_decls);
      }
    });

    if !visitor_errors.is_empty() {
      let errors = visitor_errors
        .iter()
        .map(|error| error.to_diagnostic().with_kind(self.name().into_owned()).to_color_string())
        .collect::<Vec<String>>()
        .join("\n\n");

      return Err(anyhow::anyhow!("\n{errors}"));
    }

    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}
