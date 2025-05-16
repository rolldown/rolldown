use std::borrow::Cow;

use oxc::{
  ast::{AstBuilder, ast::Expression},
  ast_visit::VisitMut,
  span::SPAN,
};
use rolldown_ecmascript_utils::ExpressionExt as _;
use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct WebWorkerPostPlugin;

impl Plugin for WebWorkerPostPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:web-worker-post")
  }

  async fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visit = WebWorkerPostVisit { ast_builder };
      visit.visit_program(fields.program);
    });
    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}

pub struct WebWorkerPostVisit<'ast> {
  pub ast_builder: AstBuilder<'ast>,
}

impl<'ast> VisitMut<'ast> for WebWorkerPostVisit<'ast> {
  fn visit_expression(&mut self, it: &mut oxc::ast::ast::Expression<'ast>) {
    if it.is_import_meta_url() {
      *it = Expression::StaticMemberExpression(self.ast_builder.alloc_static_member_expression(
        SPAN,
        Expression::StaticMemberExpression(self.ast_builder.alloc_static_member_expression(
          SPAN,
          self.ast_builder.expression_identifier(SPAN, self.ast_builder.atom("self")),
          self.ast_builder.identifier_name(SPAN, "location"),
          false,
        )),
        self.ast_builder.identifier_name(SPAN, "href"),
        false,
      ));
    }
  }
}
