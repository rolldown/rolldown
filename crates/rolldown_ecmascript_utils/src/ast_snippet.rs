use oxc::{
  allocator::{self, Allocator, Box, IntoIn, TakeIn},
  ast::{
    AstBuilder, NONE,
    ast::{
      self, Argument, ClassElement, Declaration, Expression, ImportOrExportKind, NumberBase,
      ObjectPropertyKind, PropertyKind, Statement, VariableDeclarationKind,
    },
  },
  span::{Atom, CompactStr, GetSpanMut, SPAN, Span},
  syntax::identifier,
};
use rolldown_common::{EcmaModuleAstUsage, Interop};

type PassedStr<'a> = &'a str;

// `AstBuilder` is more suitable name, but it's already used in oxc.
pub struct AstSnippet<'ast> {
  pub builder: AstBuilder<'ast>,
}

impl<'ast> AstSnippet<'ast> {
  pub fn new(alloc: &'ast Allocator) -> Self {
    Self { builder: AstBuilder::new(alloc) }
  }

  #[inline]
  pub fn alloc(&self) -> &'ast Allocator {
    self.builder.allocator
  }

  pub fn atom(&self, value: &str) -> Atom<'ast> {
    self.builder.atom(value)
  }

  #[inline]
  pub fn id(&self, name: PassedStr, span: Span) -> ast::BindingIdentifier<'ast> {
    self.builder.binding_identifier(span, self.builder.atom(name))
  }

  #[inline]
  pub fn alloc_id_ref(
    &self,
    name: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::IdentifierReference<'ast>> {
    self.builder.alloc_identifier_reference(span, self.builder.atom(name))
  }

  #[inline]
  pub fn id_name(&self, name: PassedStr, span: Span) -> ast::IdentifierName<'ast> {
    self.builder.identifier_name(span, self.builder.atom(name))
  }

  #[inline]
  pub fn id_ref_expr(&self, name: PassedStr, span: Span) -> ast::Expression<'ast> {
    self.builder.expression_identifier(span, self.builder.atom(name))
  }

  pub fn member_expr_or_ident_ref(
    &self,
    object: ast::Expression<'ast>,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    let mut cur = object;
    for name in names {
      cur = if identifier::is_identifier_name(name) {
        ast::Expression::from(self.builder.member_expression_static(
          SPAN,
          cur,
          self.id_name(name, SPAN),
          false,
        ))
      } else {
        ast::Expression::from(self.builder.member_expression_computed(
          SPAN,
          cur,
          self.builder.expression_string_literal(SPAN, self.builder.atom(name), None),
          false,
        ))
      };
    }
    *cur.span_mut() = span;
    cur
  }

  /// The props of `foo_exports.value.a` is `["value", "a"]`, here convert it to `(void 0).a`
  #[inline]
  pub fn member_expr_with_void_zero_object(
    &self,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    if names.is_empty() {
      self.void_zero()
    } else {
      self.member_expr_or_ident_ref(self.void_zero(), &names[1..], span)
    }
  }

  /// `[object].[property]`
  pub fn literal_prop_access_member_expr(
    &self,
    object: PassedStr,
    property: PassedStr,
  ) -> ast::MemberExpression<'ast> {
    ast::MemberExpression::StaticMemberExpression(self.builder.alloc_static_member_expression(
      SPAN,
      self.id_ref_expr(object, SPAN),
      self.builder.identifier_name(SPAN, self.builder.atom(property)),
      false,
    ))
  }

  /// `[object].[property]`
  #[inline]
  pub fn literal_prop_access_member_expr_expr(
    &self,
    object: PassedStr,
    property: PassedStr,
  ) -> ast::Expression<'ast> {
    ast::Expression::from(self.literal_prop_access_member_expr(object, property))
  }

  /// `name()`
  pub fn call_expr_expr(&self, name: PassedStr) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier(SPAN, self.builder.atom(name)),
      NONE,
      self.builder.vec(),
      false,
    )
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr(
    &self,
    name: ast::Expression<'ast>,
    arg: ast::Expression<'ast>,
    pure: bool,
  ) -> ast::Expression<'ast> {
    let mut call_expr = self.simple_call_expr(name);
    call_expr.pure = pure;
    call_expr.arguments.push(arg.into());
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr_expr(
    &self,
    name: PassedStr,
    arg: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let arg = ast::Argument::from(arg);
    let mut call_expr = self.builder.call_expression(
      SPAN,
      self.builder.expression_identifier(SPAN, self.builder.atom(name)),
      NONE,
      self.builder.vec(),
      false,
    );
    call_expr.arguments.push(arg);
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg1, arg2)`
  pub fn call_expr_with_2arg_expr(
    &self,
    name: ast::Expression<'ast>,
    arg1: ast::Expression<'ast>,
    arg2: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut call_expr = self.builder.call_expression(SPAN, name, NONE, self.builder.vec(), false);
    call_expr.arguments.push(arg1.into());
    call_expr.arguments.push(arg2.into());
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg1, arg2)`
  pub fn alloc_call_expr_with_2arg_expr_expr(
    &self,
    name: PassedStr,
    arg1: ast::Expression<'ast>,
    arg2: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier(SPAN, self.builder.atom(name)),
      NONE,
      self.builder.vec_from_iter([Argument::from(arg1), Argument::from(arg2)]),
      false,
    )
  }

  /// `name(arg1, arg2)`
  pub fn call_expr_with_2arg_expr_expr(
    &self,
    name: ast::Expression<'ast>,
    arg1: ast::Expression<'ast>,
    arg2: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      name,
      NONE,
      self.builder.vec_from_iter([Argument::from(arg1), Argument::from(arg2)]),
      false,
    )
  }

  /// `name()`
  #[inline]
  pub fn call_expr_stmt(&self, name: PassedStr) -> ast::Statement<'ast> {
    self.builder.statement_expression(SPAN, self.call_expr_expr(name))
  }

  /// `var [name] = [init]`
  pub fn var_decl_stmt(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Statement<'ast> {
    let declarations = self.builder.vec1(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, self.builder.atom(name)),
        NONE,
        false,
      ),
      Some(init),
      false,
    ));

    ast::Statement::from(ast::Declaration::VariableDeclaration(
      self.builder.alloc_variable_declaration(
        SPAN,
        ast::VariableDeclarationKind::Var,
        declarations,
        false,
      ),
    ))
  }

  /// ```js
  ///  var require_foo = __commonJS(((exports, module) => {
  ///    ...
  ///  }));
  ///  or
  ///  __commonJSMin when `options.profiler_names` is false
  /// ```
  pub fn commonjs_wrapper_stmt(
    &self,
    binding_name: PassedStr,
    commonjs_expr: ast::Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    ast_usage: EcmaModuleAstUsage,
    profiler_names: bool,
    stable_id: &str,
  ) -> ast::Statement<'ast> {
    // (exports, module) => {}

    let mut params = self.builder.formal_parameters(
      SPAN,
      ast::FormalParameterKind::Signature,
      self.builder.vec_with_capacity(1),
      NONE,
    );
    let body = self.builder.function_body(SPAN, self.builder.vec(), statements);
    if ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      params.items.push(self.builder.formal_parameter(
        SPAN,
        self.builder.vec(),
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, "exports"),
          NONE,
          false,
        ),
        None,
        false,
        false,
      ));
    }

    if ast_usage.contains(EcmaModuleAstUsage::ModuleRef) {
      params.items.push(self.builder.formal_parameter(
        SPAN,
        self.builder.vec(),
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, "module"),
          NONE,
          false,
        ),
        None,
        false,
        false,
      ));
    }

    //  __commonJS(...)
    let mut commonjs_call_expr = self.builder.call_expression_with_pure(
      SPAN,
      commonjs_expr,
      NONE,
      self.builder.vec(),
      false,
      true,
    );

    // the callback is marked as PIFE because most require calls are evaluated in the initial load
    let mut arrow_expr =
      self.builder.alloc_arrow_function_expression(SPAN, false, false, NONE, params, NONE, body);
    arrow_expr.pife = true;

    if profiler_names {
      let obj_expr = self.builder.alloc_object_expression(
        SPAN,
        self.builder.vec1(self.builder.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          ast::PropertyKey::from(self.builder.expression_string_literal(
            SPAN,
            self.builder.atom(stable_id),
            None,
          )),
          Expression::ArrowFunctionExpression(arrow_expr),
          true,
          false,
          false,
        )),
      );
      commonjs_call_expr.arguments.push(ast::Argument::ObjectExpression(obj_expr));
    } else {
      commonjs_call_expr.arguments.push(ast::Argument::ArrowFunctionExpression(arrow_expr));
    }

    // var require_foo = ...
    self.var_decl_stmt(
      binding_name,
      ast::Expression::CallExpression(commonjs_call_expr.into_in(self.alloc())),
    )
  }

  /// ```js
  /// var init_foo = __esm((() => { ... }));
  /// ```
  #[expect(clippy::too_many_arguments)]
  pub fn esm_wrapper_stmt(
    &self,
    binding_name: PassedStr,
    esm_fn_expr: ast::Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    profiler_names: bool,
    use_pife: bool,
    stable_id: &str,
    is_async: bool,
  ) -> ast::Statement<'ast> {
    // () => { ... }
    let params = self.builder.formal_parameters(
      SPAN,
      ast::FormalParameterKind::Signature,
      self.builder.vec(),
      NONE,
    );
    let body = self.builder.function_body(SPAN, self.builder.vec(), statements);

    //  __esm(...)
    let mut esm_call_expr =
      self.builder.call_expression(SPAN, esm_fn_expr, NONE, self.builder.vec(), false);

    // the callback is marked as PIFE because dynamically imported modules are split into a separate chunk
    // and the statically imported modules are evaluated in the initial load
    let mut arrow_expr =
      self.builder.alloc_arrow_function_expression(SPAN, false, is_async, NONE, params, NONE, body);
    arrow_expr.pife = use_pife;

    if profiler_names {
      let obj_expr = self.builder.alloc_object_expression(
        SPAN,
        self.builder.vec1(self.builder.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          ast::PropertyKey::from(self.builder.expression_string_literal(
            SPAN,
            self.builder.atom(stable_id),
            None,
          )),
          Expression::ArrowFunctionExpression(arrow_expr),
          false,
          false,
          false,
        )),
      );
      esm_call_expr.arguments.push(ast::Argument::ObjectExpression(obj_expr));
    } else {
      esm_call_expr.arguments.push(ast::Argument::ArrowFunctionExpression(arrow_expr));
    }

    // var init_foo = __esm(...)
    self.var_decl_stmt(
      binding_name,
      ast::Expression::CallExpression(esm_call_expr.into_in(self.alloc())),
    )
  }

  /// ```js
  /// (a, b)
  /// ```
  pub fn seq2_in_paren_expr(
    &self,
    a: ast::Expression<'ast>,
    b: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut expressions = self.builder.vec_with_capacity(2);
    expressions.push(a);
    expressions.push(b);
    let seq_expr = ast::Expression::SequenceExpression(
      self.builder.alloc_sequence_expression(SPAN, expressions),
    );
    ast::Expression::ParenthesizedExpression(
      self.builder.alloc_parenthesized_expression(SPAN, seq_expr),
    )
  }

  pub fn number_expr(&self, value: f64, raw: &'ast str) -> ast::Expression<'ast> {
    ast::Expression::NumericLiteral(self.builder.alloc_numeric_literal(
      SPAN,
      value,
      Some(Atom::from(raw)),
      oxc::syntax::number::NumberBase::Decimal,
    ))
  }

  /// ```js
  ///  id = ...
  /// ￣￣ AssignmentTarget
  /// ```
  pub fn simple_id_assignment_target(
    &self,
    id: PassedStr,
    span: Span,
  ) -> ast::AssignmentTarget<'ast> {
    ast::AssignmentTarget::AssignmentTargetIdentifier(self.alloc_id_ref(id, span))
  }

  /// ```js
  /// () => xx
  /// ```
  pub fn only_return_arrow_expr(&self, expr: ast::Expression<'ast>) -> ast::Expression<'ast> {
    let statements = self.builder.vec1(ast::Statement::ExpressionStatement(
      self.builder.alloc_expression_statement(SPAN, expr),
    ));
    ast::Expression::ArrowFunctionExpression(self.builder.alloc_arrow_function_expression(
      SPAN,
      true,
      false,
      NONE,
      self.builder.formal_parameters(
        SPAN,
        ast::FormalParameterKind::Signature,
        self.builder.vec(),
        NONE,
      ),
      NONE,
      self.builder.function_body(SPAN, self.builder.vec(), statements),
    ))
  }

  #[inline]
  /// `undefined` is acting like identifier, it might be shadowed by user code.
  pub fn void_zero(&self) -> ast::Expression<'ast> {
    self.builder.void_0(SPAN)
  }

  pub fn alloc_string_literal(
    &self,
    value: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::StringLiteral<'ast>> {
    self.builder.alloc_string_literal(span, self.builder.atom(value), None)
  }

  pub fn string_literal_expr(&self, value: PassedStr, span: Span) -> ast::Expression<'ast> {
    ast::Expression::StringLiteral(self.alloc_string_literal(value, span))
  }

  pub fn import_star_stmt(&self, source: PassedStr, as_name: PassedStr) -> ast::Statement<'ast> {
    let specifiers = self.builder.vec1(ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
      self.builder.alloc_import_namespace_specifier(SPAN, self.id(as_name, SPAN)),
    ));
    ast::Statement::ImportDeclaration(self.builder.alloc_import_declaration(
      SPAN,
      Some(specifiers),
      self.builder.string_literal(SPAN, self.builder.atom(source), None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }

  /// var [assignee] = require([source]);
  pub fn variable_declarator_require_call_stmt(
    &self,
    assignee: &str,
    init: ast::Expression<'ast>,
    span: Span,
  ) -> Statement<'ast> {
    Statement::from(self.builder.declaration_variable(
      span,
      VariableDeclarationKind::Var,
      self.builder.vec1(self.builder.variable_declarator(
        SPAN,
        VariableDeclarationKind::Var,
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, self.builder.atom(assignee)),
          NONE,
          false,
        ),
        Some(init),
        false,
      )),
      false,
    ))
  }

  /// Promise.resolve().then(() => expr))
  pub fn promise_resolve_then_call_expr(
    &self,
    expr: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    ast::Expression::CallExpression(self.builder.alloc_call_expression(
      SPAN,
      ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        SPAN,
        ast::Expression::CallExpression(self.builder.alloc_call_expression(
          SPAN,
          ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
            SPAN,
            self.id_ref_expr("Promise", SPAN),
            self.id_name("resolve", SPAN),
            false,
          )),
          NONE,
          self.builder.vec(),
          false,
        )),
        self.id_name("then", SPAN),
        false,
      )),
      NONE,
      self.builder.vec1(Argument::from(self.only_return_arrow_expr(expr))),
      false,
    ))
  }

  pub fn callee_then_call_expr(
    &self,
    call_expr: ast::Expression<'ast>,
    return_expr: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    ast::Expression::CallExpression(self.builder.alloc_call_expression(
      SPAN,
      ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        SPAN,
        call_expr,
        self.id_name("then", SPAN),
        false,
      )),
      NONE,
      self.builder.vec1(Argument::from(self.only_return_arrow_expr(return_expr))),
      false,
    ))
  }

  // create `a: () => expr` for  `{ a: () => expr }``
  pub fn object_property_kind_object_property(
    &self,
    key: PassedStr,
    expr: ast::Expression<'ast>,
    computed: bool,
  ) -> ObjectPropertyKind<'ast> {
    self.builder.object_property_kind_object_property(
      SPAN,
      PropertyKind::Init,
      if computed {
        ast::PropertyKey::from(self.builder.expression_string_literal(
          SPAN,
          self.builder.atom(key),
          None,
        ))
      } else {
        self.builder.property_key_static_identifier(SPAN, self.builder.atom(key))
      },
      self.only_return_arrow_expr(expr),
      true,
      false,
      computed,
    )
  }

  // If interop is None, using `require_foo()`
  // If interop is babel, using __toESM(require_foo())
  // If interop is node, using __toESM(require_foo(), 1)
  #[expect(clippy::needless_pass_by_value)]
  pub fn to_esm_call_with_interop(
    &self,
    to_esm_fn_name: PassedStr,
    call_expr: Expression<'ast>,
    interop: Option<Interop>,
  ) -> Expression<'ast> {
    match interop {
      None => call_expr,
      Some(Interop::Babel) => self.call_expr_with_arg_expr_expr(to_esm_fn_name, call_expr),
      Some(Interop::Node) => self.alloc_call_expr_with_2arg_expr_expr(
        to_esm_fn_name,
        call_expr,
        self.builder.expression_numeric_literal(SPAN, 1.0, None, NumberBase::Decimal),
      ),
    }
  }

  // If `node_mode` is true, using `__toESM(expr, 1)`
  // If `node_mode` is false, using `__toESM(expr)`
  pub fn wrap_with_to_esm(
    &self,
    to_esm_fn_expr: Expression<'ast>,
    expr: Expression<'ast>,
    node_mode: bool,
  ) -> Expression<'ast> {
    let args = if node_mode {
      self.builder.vec_from_iter([
        Argument::from(expr),
        Argument::from(self.builder.expression_numeric_literal(
          SPAN,
          1.0,
          None,
          NumberBase::Decimal,
        )),
      ])
    } else {
      self.builder.vec1(Argument::from(expr))
    };
    ast::Expression::CallExpression(self.builder.alloc_call_expression_with_pure(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
      true,
    ))
  }

  /// convert `Expression` to
  /// export default ${Expression}
  pub fn export_default_expr_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    let ast_builder = &self.builder;
    Statement::from(ast_builder.module_declaration_export_default_declaration(
      SPAN,
      ast::ExportDefaultDeclarationKind::from(expr),
    ))
  }

  /// convert `Expression` to
  /// module.exports = ${Expression}
  pub fn module_exports_expr_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    let ast_builder = &self.builder;
    ast_builder.statement_expression(
      SPAN,
      ast_builder.expression_assignment(
        SPAN,
        ast::AssignmentOperator::Assign,
        ast::AssignmentTarget::from(ast::SimpleAssignmentTarget::from(
          ast_builder.member_expression_static(
            SPAN,
            ast_builder.expression_identifier(SPAN, "module"),
            ast_builder.identifier_name(SPAN, "exports"),
            false,
          ),
        )),
        expr,
      ),
    )
  }

  pub fn expr_without_parentheses(&self, mut expr: Expression<'ast>) -> Expression<'ast> {
    while let Expression::ParenthesizedExpression(mut paren_expr) = expr {
      expr = paren_expr.expression.take_in(self.builder.allocator);
    }
    expr
  }

  #[inline]
  pub fn statement_module_declaration_export_named_declaration<T: AsRef<str>>(
    &self,
    declaration: Option<Declaration<'ast>>,
    specifiers: &[(T /*local*/, T /*exported*/, bool /*legal ident*/)],
  ) -> Statement<'ast> {
    Statement::from(self.builder.module_declaration_export_named_declaration(
      SPAN,
      declaration,
      {
        let mut vec = self.builder.vec_with_capacity(specifiers.len());
        for (local, exported, legal_ident) in specifiers {
          vec.push(
            self.builder.export_specifier(
              SPAN,
              self
                .builder
                .module_export_name_identifier_reference(SPAN, self.builder.atom(local.as_ref())),
              if *legal_ident {
                self
                  .builder
                  .module_export_name_identifier_name(SPAN, self.builder.atom(exported.as_ref()))
              } else {
                self.builder.module_export_name_string_literal(
                  SPAN,
                  self.builder.atom(exported.as_ref()),
                  None,
                )
              },
              ImportOrExportKind::Value,
            ),
          );
        }
        vec
      },
      None,
      ImportOrExportKind::Value,
      NONE,
    ))
  }

  pub fn keep_name_call_expr(
    &self,
    original_name: PassedStr,
    target: Expression<'ast>,
    callee: Expression<'ast>,
    pure: bool,
  ) -> Expression<'ast> {
    self.builder.expression_call_with_pure(
      SPAN,
      callee,
      NONE,
      {
        let mut items = self.builder.vec_with_capacity(2);
        items.push(target.into());
        items.push(
          self.builder.expression_string_literal(SPAN, self.atom(original_name), None).into(),
        );
        items
      },
      false,
      pure,
    )
  }

  pub fn static_block_keep_name_helper(
    &self,
    name: PassedStr,
    callee: Expression<'ast>,
  ) -> ClassElement<'ast> {
    self.builder.class_element_static_block(
      SPAN,
      self.builder.vec1(self.builder.statement_expression(
        SPAN,
        self.builder.expression_call(
          SPAN,
          callee,
          NONE,
          {
            let mut items = self.builder.vec_with_capacity(2);
            items.push(self.builder.expression_this(SPAN).into());
            items.push(
              self.builder.expression_string_literal(SPAN, self.builder.atom(name), None).into(),
            );
            items
          },
          false,
        ),
      )),
    )
  }

  pub fn simple_call_expr(&self, callee: Expression<'ast>) -> ast::CallExpression<'ast> {
    self.builder.call_expression(SPAN, callee, NONE, self.builder.vec(), false)
  }

  pub fn alloc_simple_call_expr(
    &self,
    callee: Expression<'ast>,
  ) -> allocator::Box<'ast, ast::CallExpression<'ast>> {
    self.builder.alloc_call_expression(SPAN, callee, NONE, self.builder.vec(), false)
  }

  pub fn object_freeze_dynamic_import_polyfill(&self) -> Expression<'ast> {
    let proto = self.builder.object_property_kind_object_property(
      SPAN,
      PropertyKind::Init,
      self.builder.property_key_static_identifier(SPAN, "__proto__"),
      ast::Expression::NullLiteral(self.builder.alloc_null_literal(SPAN)),
      false,
      false,
      false,
    );

    self.call_expr_with_arg_expr(
      self.literal_prop_access_member_expr_expr("Object", "freeze"),
      ast::Expression::ObjectExpression(
        self.builder.alloc_object_expression(SPAN, self.builder.vec_from_iter([proto])),
      ),
      true,
    )
  }
}
