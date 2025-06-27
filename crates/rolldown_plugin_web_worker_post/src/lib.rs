use std::borrow::Cow;

use oxc::{ast::ast::Expression, ast_visit::VisitMut, span::SPAN};
use rolldown_ecmascript_utils::{AstSnippet, ExpressionExt as _};
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
      let ast_snippet = AstSnippet::new(fields.allocator);
      let mut visit = WebWorkerPostVisit { ast_snippet, is_injected_import_meta: false };
      visit.visit_program(fields.program);
    });
    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
  }
}

pub struct WebWorkerPostVisit<'ast> {
  pub ast_snippet: AstSnippet<'ast>,
  pub is_injected_import_meta: bool,
}

impl<'ast> WebWorkerPostVisit<'ast> {
  #[inline]
  fn self_location_href(&self) -> Expression<'ast> {
    Expression::StaticMemberExpression(self.ast_snippet.builder.alloc_static_member_expression(
      SPAN,
      Expression::StaticMemberExpression(self.ast_snippet.builder.alloc_static_member_expression(
        SPAN,
        self.ast_snippet.id_ref_expr("self", SPAN),
        self.ast_snippet.id_name("location", SPAN),
        false,
      )),
      self.ast_snippet.id_name("href", SPAN),
      false,
    ))
  }
}

impl<'ast> VisitMut<'ast> for WebWorkerPostVisit<'ast> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'ast>) {
    oxc::ast_visit::walk_mut::walk_program(self, it);
    if self.is_injected_import_meta {
      it.body.insert(
        0,
        self.ast_snippet.var_decl_stmt(
          "_vite_importMeta",
          Expression::ObjectExpression(
            self.ast_snippet.builder.alloc_object_expression(
              SPAN,
              self.ast_snippet.builder.vec1(
                self.ast_snippet.builder.object_property_kind_object_property(
                  SPAN,
                  oxc::ast::ast::PropertyKind::Init,
                  oxc::ast::ast::PropertyKey::StaticIdentifier(
                    self
                      .ast_snippet
                      .builder
                      .alloc_identifier_name(SPAN, self.ast_snippet.builder.atom("url")),
                  ),
                  self.self_location_href(),
                  false,
                  false,
                  false,
                ),
              ),
            ),
          ),
        ),
      );
    }
  }

  fn visit_expression(&mut self, it: &mut Expression<'ast>) {
    let Expression::StaticMemberExpression(member_expr) = it else {
      return oxc::ast_visit::walk_mut::walk_expression(self, it);
    };
    if member_expr.object.is_import_meta() {
      if member_expr.property.name == "url" {
        *it = self.self_location_href();
      } else {
        self.is_injected_import_meta = true;
        member_expr.object = self.ast_snippet.id_ref_expr("_vite_importMeta", SPAN);
      }
    }
  }
}
