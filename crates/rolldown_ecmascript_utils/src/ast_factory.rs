use std::ops::Deref;

use oxc::{
  allocator::{self, Allocator, IntoIn},
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, ArrowFunctionExpression, AssignmentOperator, AssignmentTarget, BindingIdentifier,
      CallExpression, ClassElement, Declaration, ExportDefaultDeclarationKind, Expression,
      FormalParameterKind, IdentifierName, ImportDeclarationSpecifier, ImportOrExportKind,
      MemberExpression, NumberBase, ObjectPropertyKind, PropertyKey, PropertyKind,
      SimpleAssignmentTarget, Statement, VariableDeclarationKind,
    },
  },
  span::{GetSpanMut, SPAN, Span},
};
use rolldown_common::{EcmaModuleAstUsage, Interop, MemberExprProp};
use rolldown_utils::ecmascript::is_validate_identifier_name;

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

  /// `<object>.<prop>.<prop>...` — chains member access for each prop, then sets the span.
  pub fn make_member_expr_or_ident_ref(
    &self,
    object: Expression<'ast>,
    props: &[MemberExprProp],
    span: Span,
  ) -> Expression<'ast> {
    let mut cur = object;
    for prop in props {
      cur = if oxc::syntax::identifier::is_identifier_name(&prop.name) {
        Expression::from(self.member_expression_static(
          SPAN,
          cur,
          self.identifier_name(prop.span, self.str(&prop.name)),
          prop.optional,
        ))
      } else {
        Expression::from(self.member_expression_computed(
          SPAN,
          cur,
          self.expression_string_literal(prop.span, self.str(&prop.name), None),
          prop.optional,
        ))
      };
    }
    *cur.span_mut() = span;
    cur
  }

  /// The props of `foo_exports.value.a` is `["value", "a"]`; here convert it to `(void 0).a`.
  #[inline]
  pub fn make_member_expr_with_void_zero_object(
    &self,
    props: &[MemberExprProp],
    span: Span,
  ) -> Expression<'ast> {
    if props.is_empty() {
      self.void_0(SPAN)
    } else {
      self.make_member_expr_or_ident_ref(self.void_0(SPAN), &props[1..], span)
    }
  }

  /// `__reExport(<first>, <second>)` (callee provided as `re_export_fn_ref`).
  pub fn make_re_export_call(
    &self,
    re_export_fn_ref: Expression<'ast>,
    first_arg: Expression<'ast>,
    second_arg: Expression<'ast>,
  ) -> CallExpression<'ast> {
    let args = self.vec_from_iter([first_arg.into(), second_arg.into()]);
    self.call_expression(SPAN, re_export_fn_ref, NONE, args, false)
  }

  /// `<callee>(<original_name as target>, "<original_name>")`, optionally `@__PURE__`.
  pub fn make_keep_name_call(
    &self,
    original_name: &str,
    target: Expression<'ast>,
    callee: Expression<'ast>,
    pure: bool,
  ) -> Expression<'ast> {
    self.expression_call_with_pure(
      SPAN,
      callee,
      NONE,
      {
        let mut items = self.vec_with_capacity(2);
        items.push(target.into());
        items.push(self.expression_string_literal(SPAN, self.str(original_name), None).into());
        items
      },
      false,
      pure,
    )
  }

  /// `static { <callee>(this, "<name>"); }`
  pub fn make_static_block_keep_name(
    &self,
    name: &str,
    callee: Expression<'ast>,
  ) -> ClassElement<'ast> {
    self.class_element_static_block(
      SPAN,
      self.vec1(self.statement_expression(
        SPAN,
        self.expression_call(
          SPAN,
          callee,
          NONE,
          {
            let mut items = self.vec_with_capacity(2);
            items.push(self.expression_this(SPAN).into());
            items.push(self.expression_string_literal(SPAN, self.str(name), None).into());
            items
          },
          false,
        ),
      )),
    )
  }

  /// `node_mode` ? `__toESM(<expr>, 1)` : `__toESM(<expr>)` (callee `to_esm_fn_expr`, `@__PURE__`).
  pub fn make_to_esm_wrapper(
    &self,
    to_esm_fn_expr: Expression<'ast>,
    expr: Expression<'ast>,
    node_mode: bool,
  ) -> Expression<'ast> {
    let args = if node_mode {
      self.vec_from_iter([
        Argument::from(expr),
        Argument::from(self.expression_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal)),
      ])
    } else {
      self.vec1(Argument::from(expr))
    };
    Expression::CallExpression(self.alloc_call_expression_with_pure(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
      true,
    ))
  }

  /// `var <binding_name> = __commonJS(... (exports, module) => { <statements> } ...)`
  #[expect(clippy::too_many_arguments)]
  pub fn make_commonjs_wrapper_stmt(
    &self,
    binding_name: &str,
    commonjs_expr: Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    ast_usage: EcmaModuleAstUsage,
    profiler_names: bool,
    stable_id: &str,
    is_async: bool,
  ) -> Statement<'ast> {
    let mut params =
      self.formal_parameters(SPAN, FormalParameterKind::Signature, self.vec_with_capacity(1), NONE);
    let body = self.function_body(SPAN, self.vec(), statements);
    if ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      params.items.push(self.formal_parameter(
        SPAN,
        self.vec(),
        self.binding_pattern_binding_identifier(SPAN, "exports"),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
      ));
    }
    if ast_usage.contains(EcmaModuleAstUsage::ModuleRef) {
      params.items.push(self.formal_parameter(
        SPAN,
        self.vec(),
        self.binding_pattern_binding_identifier(SPAN, "module"),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
      ));
    }
    let mut commonjs_call_expr =
      self.call_expression_with_pure(SPAN, commonjs_expr, NONE, self.vec(), false, true);
    let mut arrow_expr =
      self.alloc_arrow_function_expression(SPAN, false, is_async, NONE, params, NONE, body);
    arrow_expr.pife = true;
    if profiler_names {
      let obj_expr = self.alloc_object_expression(
        SPAN,
        self.vec1(self.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          PropertyKey::from(self.expression_string_literal(SPAN, self.str(stable_id), None)),
          Expression::ArrowFunctionExpression(arrow_expr),
          true,
          false,
          false,
        )),
      );
      commonjs_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      commonjs_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    self.make_var_decl(
      binding_name,
      Expression::CallExpression(commonjs_call_expr.into_in(self.allocator)),
    )
  }

  /// `var <binding_name> = __esm(... () => { <statements> } ...)`
  #[expect(clippy::too_many_arguments)]
  pub fn make_esm_wrapper_stmt(
    &self,
    binding_name: &str,
    esm_fn_expr: Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    profiler_names: bool,
    use_pife: bool,
    stable_id: &str,
    is_async: bool,
  ) -> Statement<'ast> {
    let params = self.formal_parameters(SPAN, FormalParameterKind::Signature, self.vec(), NONE);
    let body = self.function_body(SPAN, self.vec(), statements);
    let mut esm_call_expr = self.call_expression(SPAN, esm_fn_expr, NONE, self.vec(), false);
    let mut arrow_expr =
      self.alloc_arrow_function_expression(SPAN, false, is_async, NONE, params, NONE, body);
    arrow_expr.pife = use_pife;
    if profiler_names {
      let obj_expr = self.alloc_object_expression(
        SPAN,
        self.vec1(self.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          PropertyKey::from(self.expression_string_literal(SPAN, self.str(stable_id), None)),
          Expression::ArrowFunctionExpression(arrow_expr),
          false,
          false,
          false,
        )),
      );
      esm_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      esm_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    self.make_var_decl(
      binding_name,
      Expression::CallExpression(esm_call_expr.into_in(self.allocator)),
    )
  }

  /// `n => n.<property_name>`
  fn arrow_function_extract_property(
    &self,
    property_name: &str,
  ) -> allocator::Box<'ast, ArrowFunctionExpression<'ast>> {
    debug_assert!(is_validate_identifier_name(property_name));
    self.alloc_arrow_function_expression(
      SPAN,
      true,
      false,
      NONE,
      self.formal_parameters(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        self.vec1(self.formal_parameter(
          SPAN,
          self.vec(),
          self.binding_pattern_binding_identifier(SPAN, self.str("n")),
          NONE,
          NONE,
          false,
          None,
          false,
          false,
        )),
        NONE,
      ),
      NONE,
      self.function_body(
        SPAN,
        self.vec(),
        self.vec1(Statement::ExpressionStatement(self.alloc_expression_statement(
          SPAN,
          Expression::StaticMemberExpression(self.alloc_static_member_expression(
            SPAN,
            self.expression_identifier(SPAN, "n"),
            self.identifier_name(SPAN, self.str(property_name)),
            false,
          )),
        ))),
      ),
    )
  }

  /// `<expr>.then(n => <return_expr>)`
  fn then_with_arrow_callback(
    &self,
    expr: Expression<'ast>,
    return_expr: Expression<'ast>,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let arrow_fn = self.alloc_arrow_function_expression(
      SPAN,
      true,
      false,
      NONE,
      self.formal_parameters(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        self.vec1(self.formal_parameter(
          SPAN,
          self.vec(),
          self.binding_pattern_binding_identifier(SPAN, self.str("n")),
          NONE,
          NONE,
          false,
          None,
          false,
          false,
        )),
        NONE,
      ),
      NONE,
      self.function_body(
        SPAN,
        self.vec(),
        self
          .vec1(Statement::ExpressionStatement(self.alloc_expression_statement(SPAN, return_expr))),
      ),
    );
    let callee =
      self.alloc_static_member_expression(SPAN, expr, self.identifier_name(SPAN, "then"), false);
    self.alloc_call_expression(
      SPAN,
      Expression::StaticMemberExpression(callee),
      NONE,
      self.vec1(Expression::ArrowFunctionExpression(arrow_fn).into()),
      false,
    )
  }

  /// `<expr>.then(n => n.<property_name>)`
  pub fn make_then_extract_property(
    &self,
    expr: Expression<'ast>,
    property_name: &str,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let callee =
      self.alloc_static_member_expression(SPAN, expr, self.identifier_name(SPAN, "then"), false);
    self.alloc_call_expression(
      SPAN,
      Expression::StaticMemberExpression(callee),
      NONE,
      self.vec1(
        Expression::ArrowFunctionExpression(self.arrow_function_extract_property(property_name))
          .into(),
      ),
      false,
    )
  }

  /// `<expr>.then(n => (n.<wrapper_name>(), n.<namespace_name>))`
  pub fn make_then_call_esm_wrapper_with_namespace(
    &self,
    expr: Expression<'ast>,
    wrapper_name: &str,
    namespace_name: &str,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let wrapper_member = Expression::StaticMemberExpression(self.alloc_static_member_expression(
      SPAN,
      self.expression_identifier(SPAN, "n"),
      self.identifier_name(SPAN, self.str(wrapper_name)),
      false,
    ));
    let wrapper_call = self.expression_call(SPAN, wrapper_member, NONE, self.vec(), false);
    let namespace_member = Expression::StaticMemberExpression(self.alloc_static_member_expression(
      SPAN,
      self.expression_identifier(SPAN, "n"),
      self.identifier_name(SPAN, self.str(namespace_name)),
      false,
    ));
    let seq_expr = self.make_seq_in_parens(wrapper_call, namespace_member);
    self.then_with_arrow_callback(expr, seq_expr)
  }

  /// `<expr>.then(n => __toESM(n.<property_name>()))`
  pub fn make_then_call_cjs_wrapper_with_to_esm(
    &self,
    expr: Expression<'ast>,
    property_name: &str,
    to_esm_fn_expr: Expression<'ast>,
    node_mode: bool,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let member_expr = Expression::StaticMemberExpression(self.alloc_static_member_expression(
      SPAN,
      self.expression_identifier(SPAN, "n"),
      self.identifier_name(SPAN, self.str(property_name)),
      false,
    ));
    let wrapper_call = self.expression_call(SPAN, member_expr, NONE, self.vec(), false);
    let to_esm_call = self.make_to_esm_wrapper(to_esm_fn_expr, wrapper_call, node_mode);
    self.then_with_arrow_callback(expr, to_esm_call)
  }

  /// `<call_expr>.then(() => <return_expr>)`
  pub fn make_callee_then_call(
    &self,
    call_expr: Expression<'ast>,
    return_expr: Expression<'ast>,
  ) -> Expression<'ast> {
    Expression::CallExpression(self.alloc_call_expression(
      SPAN,
      Expression::StaticMemberExpression(self.alloc_static_member_expression(
        SPAN,
        call_expr,
        self.identifier_name(SPAN, "then"),
        false,
      )),
      NONE,
      self.vec1(Argument::from(self.make_arrow_returning(return_expr))),
      false,
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
