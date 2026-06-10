use std::ops::Deref;

use oxc::{
  allocator::{Allocator, IntoIn},
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, AssignmentOperator, AssignmentTarget, BindingIdentifier, Declaration,
      ExportDefaultDeclarationKind, Expression, FormalParameterKind, IdentifierName,
      ImportDeclarationSpecifier, ImportOrExportKind, MemberExpression, NumberBase,
      ObjectPropertyKind, PropertyKey, PropertyKind, SimpleAssignmentTarget, Statement,
      VariableDeclarationKind,
    },
  },
  span::{SPAN, Span},
};
use rolldown_common::Interop;

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

  /// `<name>` as a `BindingIdentifier`, with the name copied into the arena.
  pub fn make_id(&self, span: Span, name: &str) -> BindingIdentifier<'ast> {
    self.binding_identifier(span, self.str(name))
  }

  /// A reference to `<name>` as an `Expression`, with the name copied into the arena.
  pub fn make_id_ref_expr(&self, span: Span, name: &str) -> Expression<'ast> {
    self.expression_identifier(span, self.str(name))
  }

  /// `<name>` as an `IdentifierName`, with the name copied into the arena.
  pub fn make_id_name(&self, span: Span, name: &str) -> IdentifierName<'ast> {
    self.identifier_name(span, self.str(name))
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

  /// `() => <expr>`
  pub fn make_arrow_returning(&self, expr: Expression<'ast>) -> Expression<'ast> {
    let statements =
      self.vec1(Statement::ExpressionStatement(self.alloc_expression_statement(SPAN, expr)));
    Expression::ArrowFunctionExpression(self.alloc_arrow_function_expression(
      SPAN,
      true,
      false,
      NONE,
      self.formal_parameters(SPAN, FormalParameterKind::Signature, self.vec(), NONE),
      NONE,
      self.function_body(SPAN, self.vec(), statements),
    ))
  }

  /// `<object>.<property>` as a `MemberExpression`.
  pub fn make_member_access(&self, object: &str, property: &str) -> MemberExpression<'ast> {
    MemberExpression::StaticMemberExpression(self.alloc_static_member_expression(
      SPAN,
      self.make_id_ref_expr(SPAN, object),
      self.make_id_name(SPAN, property),
      false,
    ))
  }

  /// `<object>.<property>` as an `Expression`.
  pub fn make_member_access_expr(&self, object: &str, property: &str) -> Expression<'ast> {
    Expression::from(self.make_member_access(object, property))
  }

  /// `<callee>(<arg>)`, optionally annotated `@__PURE__`.
  pub fn make_call_with_arg(
    &self,
    callee: Expression<'ast>,
    arg: Expression<'ast>,
    pure: bool,
  ) -> Expression<'ast> {
    let mut call_expr = self.call_expression(SPAN, callee, NONE, self.vec(), false);
    call_expr.pure = pure;
    call_expr.arguments.push(arg.into());
    Expression::CallExpression(call_expr.into_in(self.allocator))
  }

  /// `Promise.resolve().then(() => <expr>)`
  pub fn make_promise_resolve_then(&self, expr: Expression<'ast>) -> Expression<'ast> {
    Expression::CallExpression(self.alloc_call_expression(
      SPAN,
      Expression::StaticMemberExpression(self.alloc_static_member_expression(
        SPAN,
        Expression::CallExpression(self.alloc_call_expression(
          SPAN,
          Expression::StaticMemberExpression(self.alloc_static_member_expression(
            SPAN,
            self.expression_identifier(SPAN, self.str("Promise")),
            self.identifier_name(SPAN, self.str("resolve")),
            false,
          )),
          NONE,
          self.vec(),
          false,
        )),
        self.identifier_name(SPAN, self.str("then")),
        false,
      )),
      NONE,
      self.vec1(Argument::from(self.make_arrow_returning(expr))),
      false,
    ))
  }

  /// `<key>: () => <expr>` — a lazy-export object property.
  pub fn make_lazy_export_property(
    &self,
    key: &str,
    expr: Expression<'ast>,
    computed: bool,
  ) -> ObjectPropertyKind<'ast> {
    self.object_property_kind_object_property(
      SPAN,
      PropertyKind::Init,
      if computed {
        PropertyKey::from(self.expression_string_literal(SPAN, self.str(key), None))
      } else {
        self.property_key_static_identifier(SPAN, self.str(key))
      },
      self.make_arrow_returning(expr),
      true,
      false,
      computed,
    )
  }

  /// `import * as <as_name> from "<source>";`
  pub fn make_import_star_stmt(&self, source: &str, as_name: &str) -> Statement<'ast> {
    let specifiers = self.vec1(ImportDeclarationSpecifier::ImportNamespaceSpecifier(
      self.alloc_import_namespace_specifier(SPAN, self.binding_identifier(SPAN, self.str(as_name))),
    ));
    Statement::ImportDeclaration(self.alloc_import_declaration(
      SPAN,
      Some(specifiers),
      self.string_literal(SPAN, self.str(source), None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }

  /// `None` → `<call_expr>`; `Babel` → `__toESM(<call_expr>)`; `Node` → `__toESM(<call_expr>, 1)`.
  #[expect(clippy::needless_pass_by_value)]
  pub fn make_to_esm_call_with_interop(
    &self,
    to_esm_fn_name: &str,
    call_expr: Expression<'ast>,
    interop: Option<Interop>,
  ) -> Expression<'ast> {
    let arguments = match interop {
      None => return call_expr,
      Some(Interop::Babel) => self.vec1(Argument::from(call_expr)),
      Some(Interop::Node) => self.vec_from_iter([
        Argument::from(call_expr),
        Argument::from(self.expression_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal)),
      ]),
    };
    self.expression_call(
      SPAN,
      self.expression_identifier(SPAN, self.str(to_esm_fn_name)),
      NONE,
      arguments,
      false,
    )
  }

  /// `(<a>, <b>)` — a parenthesized two-element sequence expression.
  pub fn make_seq_in_parens(&self, a: Expression<'ast>, b: Expression<'ast>) -> Expression<'ast> {
    let mut expressions = self.vec_with_capacity(2);
    expressions.push(a);
    expressions.push(b);
    Expression::ParenthesizedExpression(self.alloc_parenthesized_expression(
      SPAN,
      Expression::SequenceExpression(self.alloc_sequence_expression(SPAN, expressions)),
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
