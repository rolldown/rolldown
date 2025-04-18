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
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let id = args.id.to_slash_lossy();
      let root = self.config.root.as_ref().map(PathBuf::from);
      let root = root.as_ref().unwrap_or(args.cwd);

      let ast_builder = AstBuilder::new(fields.allocator);

      let mut visitor = GlobImportVisit {
        id,
        root,
        ast_builder,
        restore_query_extension: self.config.restore_query_extension,
        current: 0,
        import_decls: ast_builder.vec(),
      };

      visitor.visit_program(fields.program);
      if !visitor.import_decls.is_empty() {
        fields.program.body.extend(visitor.import_decls);
      }
    });
    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}
