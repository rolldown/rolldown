use oxc::{ast::ast::Expression, ast_visit::VisitMut, span::SPAN};
use rolldown_ecmascript_utils::{AstSnippet, ExpressionExt as _};

pub struct WebWorkerPostVisitor<'ast> {
  pub ast_snippet: AstSnippet<'ast>,
  pub should_inject_import_meta_object: bool,
}

impl<'ast> WebWorkerPostVisitor<'ast> {
  pub fn new(ast_snippet: AstSnippet<'ast>) -> Self {
    Self { ast_snippet, should_inject_import_meta_object: false }
  }

  #[inline]
  fn create_self_location_href_expr(&self) -> Expression<'ast> {
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

  #[inline]
  fn create_import_meta_object_decl(&self) -> oxc::ast::ast::Statement<'ast> {
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
              self.create_self_location_href_expr(),
              false,
              false,
              false,
            ),
          ),
        ),
      ),
    )
  }
}

impl<'ast> VisitMut<'ast> for WebWorkerPostVisitor<'ast> {
  fn visit_program(&mut self, it: &mut oxc::ast::ast::Program<'ast>) {
    oxc::ast_visit::walk_mut::walk_program(self, it);
    if self.should_inject_import_meta_object {
      it.body.insert(0, self.create_import_meta_object_decl());
    }
  }

  fn visit_expression(&mut self, it: &mut Expression<'ast>) {
    match it {
      Expression::StaticMemberExpression(member_expr) if member_expr.object.is_import_meta() => {
        if member_expr.property.name == "url" {
          *it = self.create_self_location_href_expr();
        }
      }
      Expression::MetaProperty(_) => {
        self.should_inject_import_meta_object = true;
        *it = self.ast_snippet.id_ref_expr("_vite_importMeta", SPAN);
      }
      _ => oxc::ast_visit::walk_mut::walk_expression(self, it),
    }
  }
}
