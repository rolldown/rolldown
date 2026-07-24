use oxc::ast::builder::AstBuilder;
use oxc::{
  ast::ast::{Expression, IdentifierName, ObjectPropertyKind, Statement},
  ast_visit::VisitMut,
  span::SPAN,
};
use rolldown_ecmascript_utils::{
  ExpressionExt as _, ExpressionFactoryExt as _, IdentifierNameFactoryExt as _,
  StatementFactoryExt as _,
};

pub struct WebWorkerPostVisitor<'ast> {
  pub ast_builder: AstBuilder<'ast>,
  pub should_inject_import_meta_object: bool,
}

impl<'ast> WebWorkerPostVisitor<'ast> {
  pub fn new(ast_builder: AstBuilder<'ast>) -> Self {
    Self { ast_builder, should_inject_import_meta_object: false }
  }

  #[inline]
  fn create_self_location_href_expr(&self) -> Expression<'ast> {
    Expression::new_static_member_expression(
      SPAN,
      Expression::new_static_member_expression(
        SPAN,
        Expression::new_id_ref_expr(SPAN, "self", &self.ast_builder),
        IdentifierName::new_id_name(SPAN, "location", &self.ast_builder),
        false,
        &self.ast_builder,
      ),
      IdentifierName::new_id_name(SPAN, "href", &self.ast_builder),
      false,
      &self.ast_builder,
    )
  }

  #[inline]
  fn create_import_meta_object_decl(&self) -> oxc::ast::ast::Statement<'ast> {
    Statement::new_var_decl(
      "_vite_importMeta",
      Expression::new_object_expression(
        SPAN,
        oxc::allocator::Vec::from_value_in(
          ObjectPropertyKind::new_object_property(
            SPAN,
            oxc::ast::ast::PropertyKind::Init,
            oxc::ast::ast::PropertyKey::new_static_identifier(
              SPAN,
              oxc::ast::ast::Str::from_str_in("url", &self.ast_builder),
              &self.ast_builder,
            ),
            self.create_self_location_href_expr(),
            false,
            false,
            false,
            &self.ast_builder,
          ),
          &self.ast_builder,
        ),
        &self.ast_builder,
      ),
      &self.ast_builder,
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
      Expression::ImportMeta(_) => {
        self.should_inject_import_meta_object = true;
        *it = Expression::new_id_ref_expr(SPAN, "_vite_importMeta", &self.ast_builder);
      }
      _ => oxc::ast_visit::walk_mut::walk_expression(self, it),
    }
  }
}
