use std::borrow::Cow;

use oxc::{
  allocator::IntoIn,
  ast_visit::VisitMut,
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
};
use rolldown_common::ModuleType;
use rolldown_plugin::{Plugin, PluginHookMeta, PluginOrder};
use type_import_visitor::TypeImportVisitor;

mod type_import_visitor;

#[derive(Debug, Default)]
pub struct DtsPlugin {
  pub strip_internal: bool,
}

impl Plugin for DtsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dts")
  }

  async fn transform_ast(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) {
      let type_import_specifiers = args.ast.program.with_mut(|fields| {
        let mut visitor = TypeImportVisitor { imported: vec![].into_in(fields.allocator) };
        visitor.visit_program(fields.program);
        visitor.imported
      });

      for specifier in type_import_specifiers {
        let resolved_id = ctx.resolve(&specifier, Some(args.id), None).await??;
        ctx.load(&resolved_id.id, None, None).await?;
      }

      let ret = args.ast.program.with_mut(|fields| {
        IsolatedDeclarations::new(
          fields.allocator,
          IsolatedDeclarationsOptions { strip_internal: self.strip_internal },
        )
        .build(fields.program)
      });

      // TODO BuildDiagnostic error
      if !ret.errors.is_empty() {
        return Err(anyhow::anyhow!("IsolatedDeclarations error"));
      }
    }
    Ok(args.ast)
  }

  // The rolldown strip types at the end of the build process, make sure to run this plugin before that.
  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }
}
