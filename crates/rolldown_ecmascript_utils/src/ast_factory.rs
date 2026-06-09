use std::ops::Deref;

use oxc::{
  allocator::Allocator,
  ast::{
    AstBuilder, NONE,
    ast::{
      AssignmentOperator, AssignmentTarget, Declaration, ExportDefaultDeclarationKind, Expression,
      ImportOrExportKind, SimpleAssignmentTarget, Statement, VariableDeclarationKind,
    },
  },
  span::SPAN,
};

/// Rolldown's newtype wrapper around oxc's [`AstBuilder`].
///
/// Generic oxc node constructors are reached through [`Deref`]; rolldown's own
/// recurring constructions are added as inherent `make_*` methods. Routing all
/// construction through this single type lets rolldown absorb future oxc
/// construction-API changes at one point.
///
/// See `meta/design/ast-construction.md`.
#[derive(Clone, Copy)]
pub struct AstFactory<'ast>(AstBuilder<'ast>);

impl<'ast> AstFactory<'ast> {
  pub fn new(allocator: &'ast Allocator) -> Self {
    Self(AstBuilder::new(allocator))
  }

  /// `var <name> = <init>;`
  pub fn make_var_decl(&self, name: &str, init: Expression<'ast>) -> Statement<'ast> {
    let declarations = self.vec1(self.variable_declarator(
      SPAN,
      VariableDeclarationKind::Var,
      self.binding_pattern_binding_identifier(SPAN, self.str(name)),
      NONE,
      Some(init),
      false,
    ));

    Statement::from(Declaration::VariableDeclaration(self.alloc_variable_declaration(
      SPAN,
      VariableDeclarationKind::Var,
      declarations,
      false,
    )))
  }

  /// `export default <expr>`
  pub fn make_export_default_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    Statement::from(self.module_declaration_export_default_declaration(
      SPAN,
      ExportDefaultDeclarationKind::from(expr),
    ))
  }

  /// `module.exports = <expr>`
  pub fn make_module_exports_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    self.statement_expression(
      SPAN,
      self.expression_assignment(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::from(SimpleAssignmentTarget::from(self.member_expression_static(
          SPAN,
          self.expression_identifier(SPAN, "module"),
          self.identifier_name(SPAN, "exports"),
          false,
        ))),
        expr,
      ),
    )
  }

  /// `export { <local> as <exported>, ... };`, optionally with a `declaration`.
  pub fn make_export_named_stmt<'a, T: AsRef<str> + 'a, I>(
    &self,
    declaration: Option<Declaration<'ast>>,
    specifiers: I,
  ) -> Statement<'ast>
  where
    I: Iterator<Item = (&'a T, &'a (T, bool))>,
  {
    Statement::from(self.module_declaration_export_named_declaration(
      SPAN,
      declaration,
      self.vec_from_iter(specifiers.into_iter().map(|(local, (exported, legal_ident))| {
        self.export_specifier(
          SPAN,
          self.module_export_name_identifier_reference(SPAN, self.str(local.as_ref())),
          if *legal_ident {
            self.module_export_name_identifier_name(SPAN, self.str(exported.as_ref()))
          } else {
            self.module_export_name_string_literal(SPAN, self.str(exported.as_ref()), None)
          },
          ImportOrExportKind::Value,
        )
      })),
      None,
      ImportOrExportKind::Value,
      NONE,
    ))
  }
}

impl<'ast> Deref for AstFactory<'ast> {
  type Target = AstBuilder<'ast>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
