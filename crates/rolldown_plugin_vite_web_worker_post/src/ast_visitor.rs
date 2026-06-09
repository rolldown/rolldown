use oxc::{ast::ast::Expression, ast_visit::VisitMut, span::SPAN};
use rolldown_ecmascript_utils::{AstFactory, ExpressionExt as _};

pub struct WebWorkerPostVisitor<'ast> {
  pub ast_factory: AstFactory<'ast>,
  pub should_inject_import_meta_object: bool,
}

impl<'ast> WebWorkerPostVisitor<'ast> {
  pub fn new(ast_factory: AstFactory<'ast>) -> Self {
    Self { ast_factory, should_inject_import_meta_object: false }
  }

  #[inline]
  fn create_self_location_href_expr(&self) -> Expression<'ast> {
    Expression::StaticMemberExpression(self.ast_factory.alloc_static_member_expression(
      SPAN,
      Expression::StaticMemberExpression(self.ast_factory.alloc_static_member_expression(
        SPAN,
        self.ast_factory.expression_identifier(SPAN, self.ast_factory.str("self")),
        self.ast_factory.identifier_name(SPAN, self.ast_factory.str("location")),
        false,
      )),
      self.ast_factory.identifier_name(SPAN, self.ast_factory.str("href")),
      false,
    ))
  }

  #[inline]
  fn create_import_meta_object_decl(&self) -> oxc::ast::ast::Statement<'ast> {
    self.ast_factory.make_var_decl(
      "_vite_importMeta",
      Expression::ObjectExpression(self.ast_factory.alloc_object_expression(
        SPAN,
        self.ast_factory.vec1(self.ast_factory.object_property_kind_object_property(
          SPAN,
          oxc::ast::ast::PropertyKind::Init,
          oxc::ast::ast::PropertyKey::StaticIdentifier(
            self.ast_factory.alloc_identifier_name(SPAN, self.ast_factory.str("url")),
          ),
          self.create_self_location_href_expr(),
          false,
          false,
          false,
        )),
      )),
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
      Expression::MetaProperty(meta)
        if meta.meta.name == "import" && meta.property.name == "meta" =>
      {
        self.should_inject_import_meta_object = true;
        *it =
          self.ast_factory.expression_identifier(SPAN, self.ast_factory.str("_vite_importMeta"));
      }
      _ => oxc::ast_visit::walk_mut::walk_expression(self, it),
    }
  }
}
