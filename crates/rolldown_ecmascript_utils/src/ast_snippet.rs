use oxc::{
  allocator::{self, Allocator, Box, IntoIn},
  ast::{
    ast::{
      self, Argument, BindingIdentifier, ClassElement, Declaration, Expression, FunctionType,
      ImportOrExportKind, NumberBase, ObjectPropertyKind, PropertyKind, Statement,
      VariableDeclarationKind,
    },
    AstBuilder, NONE,
  },
  span::{Atom, CompactStr, Span, SPAN},
};
use rolldown_common::{EcmaModuleAstUsage, Interop};

use crate::allocator_helpers::take_in::TakeIn;

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
    self.builder.binding_identifier(span, name)
  }

  #[inline]
  pub fn alloc_id_ref(
    &self,
    name: PassedStr,
    span: Span,
  ) -> Box<'ast, ast::IdentifierReference<'ast>> {
    self.builder.alloc_identifier_reference(span, name)
  }

  #[inline]
  pub fn id_name(&self, name: PassedStr, span: Span) -> ast::IdentifierName<'ast> {
    self.builder.identifier_name(span, name)
  }

  #[inline]
  pub fn id_ref_expr(&self, name: PassedStr, span: Span) -> ast::Expression<'ast> {
    self.builder.expression_identifier_reference(span, name)
  }

  pub fn member_expr_or_ident_ref(
    &self,
    object: ast::Expression<'ast>,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    match names {
      [] => object,
      _ => ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        span,
        self.member_expr_or_ident_ref(object, &names[0..names.len() - 1], span),
        self.id_name(names[names.len() - 1].as_str(), span),
        false,
      )),
    }
  }

  /// The props of `foo_exports.value.a` is `["value", "a"]`, here convert it to `(void 0).a`
  pub fn member_expr_with_void_zero_object(
    &self,
    names: &[CompactStr],
    span: Span,
  ) -> ast::Expression<'ast> {
    if names.len() == 1 {
      self.void_zero()
    } else {
      ast::Expression::StaticMemberExpression(self.builder.alloc_static_member_expression(
        span,
        self.member_expr_with_void_zero_object(&names[0..names.len() - 1], span),
        self.id_name(names[names.len() - 1].as_str(), span),
        false,
      ))
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
      self.builder.identifier_name(SPAN, property),
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
  #[inline]
  pub fn call_expr(&self, name: PassedStr) -> ast::CallExpression<'ast> {
    self.builder.call_expression(
      SPAN,
      self.builder.expression_identifier_reference(SPAN, name),
      NONE,
      self.builder.vec(),
      false,
    )
  }

  /// `name()`
  pub fn call_expr_expr(&self, name: PassedStr) -> ast::Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier_reference(SPAN, name),
      NONE,
      self.builder.vec(),
      false,
    )
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr(&self, name: PassedStr, arg: PassedStr) -> ast::Expression<'ast> {
    let arg = ast::Argument::Identifier(self.alloc_id_ref(arg, SPAN));
    let mut call_expr = self.call_expr(name);
    call_expr.arguments.push(arg);
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr_expr(
    &self,
    name: PassedStr,
    arg: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let arg = ast::Argument::from(arg);
    let mut call_expr = self.call_expr(name);
    call_expr.arguments.push(arg);
    ast::Expression::CallExpression(call_expr.into_in(self.alloc()))
  }

  /// `name(arg1, arg2)`
  pub fn call_expr_with_2arg_expr(
    &self,
    name: PassedStr,
    arg1: PassedStr,
    arg2: PassedStr,
  ) -> ast::Expression<'ast> {
    let arg1 = ast::Argument::Identifier(self.builder.alloc_identifier_reference(SPAN, arg1));
    let arg2 = ast::Argument::Identifier(self.builder.alloc_identifier_reference(SPAN, arg2));
    let mut call_expr = self.call_expr(name);
    call_expr.arguments.push(arg1);
    call_expr.arguments.push(arg2);
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
      self.builder.expression_identifier_reference(SPAN, name),
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
  #[inline]
  pub fn var_decl_stmt(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Statement<'ast> {
    ast::Statement::from(self.decl_var_decl(name, init))
  }

  /// `var [name] = [init]`
  pub fn decl_var_decl(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> ast::Declaration<'ast> {
    let declarations = self.builder.vec1(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, name),
        NONE,
        false,
      ),
      Some(init),
      false,
    ));

    ast::Declaration::VariableDeclaration(self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    ))
  }

  /// `var [name] = [init]`
  pub fn var_decl(
    &self,
    name: PassedStr,
    init: ast::Expression<'ast>,
  ) -> Box<'ast, ast::VariableDeclaration<'ast>> {
    let declarations = self.builder.vec1(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, name),
        NONE,
        false,
      ),
      Some(init),
      false,
    ));
    self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    )
  }

  pub fn var_decl_multiple_names(
    &self,
    names: &[(&str, &str)],
    init: ast::Expression<'ast>,
  ) -> Box<'ast, ast::VariableDeclaration<'ast>> {
    let mut declarations = self.builder.vec_with_capacity(1);
    let mut properties = self.builder.vec();
    names.iter().for_each(|(imported, local)| {
      properties.push(self.builder.binding_property(
        SPAN,
        self.builder.property_key_identifier_name(SPAN, *imported),
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, *local),
          NONE,
          false,
        ),
        false,
        false,
      ));
    });
    declarations.push(ast::VariableDeclarator {
      id: ast::BindingPattern {
        kind: ast::BindingPatternKind::ObjectPattern(
          ast::ObjectPattern { properties, ..TakeIn::dummy(self.alloc()) }.into_in(self.alloc()),
        ),
        ..TakeIn::dummy(self.alloc())
      },
      init: Some(init),
      ..TakeIn::dummy(self.alloc())
    });
    self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    )
  }

  /// ```js
  ///  var require_foo = __commonJS((exports, module) => {
  ///    ...
  ///  });
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
    let mut commonjs_call_expr =
      self.builder.call_expression(SPAN, commonjs_expr, NONE, self.builder.vec(), false);
    if profiler_names {
      let obj_expr = self.builder.alloc_object_expression(
        SPAN,
        self.builder.vec1(self.builder.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          ast::PropertyKey::from(self.builder.expression_string_literal(SPAN, stable_id, None)),
          self.builder.expression_function(
            SPAN,
            FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            params,
            NONE,
            Some(body),
          ),
          true,
          false,
          false,
        )),
        None,
      );
      commonjs_call_expr.arguments.push(ast::Argument::ObjectExpression(obj_expr));
    } else {
      let arrow_expr =
        self.builder.alloc_arrow_function_expression(SPAN, false, false, NONE, params, NONE, body);
      commonjs_call_expr.arguments.push(ast::Argument::ArrowFunctionExpression(arrow_expr));
    };

    // var require_foo = ...
    let var_decl_stmt = self.var_decl_stmt(
      binding_name,
      ast::Expression::CallExpression(commonjs_call_expr.into_in(self.alloc())),
    );

    var_decl_stmt
  }

  /// ```js
  /// var init_foo = __esm(() => { ... });
  /// ```
  pub fn esm_wrapper_stmt(
    &self,
    binding_name: PassedStr,
    esm_fn_expr: ast::Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    profiler_names: bool,
    stable_id: &str,
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

    if profiler_names {
      let obj_expr = self.builder.alloc_object_expression(
        SPAN,
        self.builder.vec1(self.builder.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          ast::PropertyKey::from(self.builder.expression_string_literal(SPAN, stable_id, None)),
          self.builder.expression_function(
            SPAN,
            FunctionType::FunctionExpression,
            None,
            false,
            false,
            false,
            NONE,
            NONE,
            params,
            NONE,
            Some(body),
          ),
          true,
          false,
          false,
        )),
        None,
      );
      esm_call_expr.arguments.push(ast::Argument::ObjectExpression(obj_expr));
    } else {
      let arrow_expr =
        self.builder.alloc_arrow_function_expression(SPAN, false, false, NONE, params, NONE, body);
      esm_call_expr.arguments.push(ast::Argument::ArrowFunctionExpression(arrow_expr));
    };

    // var init_foo = ...

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
    self.builder.alloc_string_literal(span, value, None)
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
      self.builder.string_literal(SPAN, source, None),
      None,
      NONE,
      ImportOrExportKind::Value,
    ))
  }

  pub fn app_static_import_star_call_stmt(
    &self,
    as_name: &str,
    importee_source: &str,
  ) -> ast::Statement<'ast> {
    let mut declarations = allocator::Vec::new_in(self.alloc());

    let mut call_expr = self.call_expr("__static_import");
    call_expr
      .arguments
      .push(ast::Argument::StringLiteral(self.alloc_string_literal(importee_source, SPAN)));
    declarations.push(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_binding_identifier(SPAN, as_name),
        NONE,
        false,
      ),
      Some(ast::Expression::CallExpression(call_expr.into_in(self.alloc()))),
      false,
    ));

    ast::Statement::VariableDeclaration(self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    ))
  }

  pub fn app_static_import_call_multiple_specifiers_stmt(
    &self,
    names: &[(&str, &str)],
    importee_source: &str,
  ) -> ast::Statement<'ast> {
    let mut declarations = self.builder.vec();
    let mut properties = self.builder.vec();
    names.iter().for_each(|(imported, local)| {
      properties.push(self.builder.binding_property(
        SPAN,
        self.builder.property_key_identifier_name(SPAN, *imported),
        self.builder.binding_pattern(
          self.builder.binding_pattern_kind_binding_identifier(SPAN, *local),
          NONE,
          false,
        ),
        false,
        false,
      ));
    });
    let mut call_expr = self.call_expr("__static_import");
    call_expr
      .arguments
      .push(ast::Argument::StringLiteral(self.alloc_string_literal(importee_source, SPAN)));
    declarations.push(self.builder.variable_declarator(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.builder.binding_pattern(
        self.builder.binding_pattern_kind_object_pattern(SPAN, properties, NONE),
        NONE,
        false,
      ),
      Some(ast::Expression::CallExpression(call_expr.into_in(self.alloc()))),
      false,
    ));

    ast::Statement::VariableDeclaration(self.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      declarations,
      false,
    ))
  }

  pub fn require_call_expr(&self, source: &str) -> Expression<'ast> {
    self.builder.expression_call(
      SPAN,
      self.builder.expression_identifier_reference(SPAN, "require"),
      NONE,
      self.builder.vec1(Argument::from(self.builder.expression_string_literal(SPAN, source, None))),
      false,
    )
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
          self.builder.binding_pattern_kind_binding_identifier(SPAN, assignee),
          NONE,
          false,
        ),
        Some(init),
        false,
      )),
      false,
    ))
  }

  /// Promise.resolve().then(function() {})
  pub fn promise_resolve_then_call_expr(
    &self,
    span: Span,
    statements: allocator::Vec<'ast, Statement<'ast>>,
  ) -> ast::Expression<'ast> {
    let arguments = self.builder.vec1(Argument::FunctionExpression(self.builder.alloc_function(
      SPAN,
      ast::FunctionType::FunctionExpression,
      None::<BindingIdentifier>,
      false,
      false,
      false,
      NONE,
      NONE,
      self.builder.formal_parameters(
        SPAN,
        ast::FormalParameterKind::Signature,
        self.builder.vec_with_capacity(2),
        NONE,
      ),
      NONE,
      Some(self.builder.function_body(SPAN, self.builder.vec(), statements)),
    )));

    let callee =
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
      ));
    ast::Expression::CallExpression(
      self.builder.alloc_call_expression(span, callee, NONE, arguments, false),
    )
  }

  // return xxx
  pub fn return_stmt(&self, argument: ast::Expression<'ast>) -> ast::Statement<'ast> {
    ast::Statement::ReturnStatement(
      ast::ReturnStatement { argument: Some(argument), ..TakeIn::dummy(self.alloc()) }
        .into_in(self.alloc()),
    )
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
        ast::PropertyKey::from(self.builder.expression_string_literal(SPAN, key, None))
      } else {
        self.builder.property_key_identifier_name(SPAN, key)
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
  #[allow(clippy::needless_pass_by_value)]
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
    ast::Expression::CallExpression(self.builder.alloc_call_expression(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
    ))
  }

  /// convert `Expression` to
  /// export default ${Expression}
  pub fn export_default_expr_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    let ast_builder = &self.builder;
    Statement::from(ast_builder.module_declaration_export_default_declaration(
      SPAN,
      ast::ExportDefaultDeclarationKind::from(expr),
      ast_builder.module_export_name_identifier_name(SPAN, "default"),
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
            ast_builder.expression_identifier_reference(SPAN, "module"),
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
      expr = self.builder.move_expression(&mut paren_expr.expression);
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
          vec.push(self.builder.export_specifier(
            SPAN,
            self.builder.module_export_name_identifier_reference(SPAN, local.as_ref()),
            if *legal_ident {
              self.builder.module_export_name_identifier_name(SPAN, exported.as_ref())
            } else {
              self.builder.module_export_name_string_literal(SPAN, exported.as_ref(), None)
            },
            ImportOrExportKind::Value,
          ));
        }
        vec
      },
      None,
      ImportOrExportKind::Value,
      NONE,
    ))
  }

  pub fn keep_name_call_expr_stmt(
    &self,
    original_name: PassedStr,
    new_name: PassedStr,
  ) -> Statement<'ast> {
    self.builder.statement_expression(
      SPAN,
      self.builder.expression_call(
        SPAN,
        self.builder.expression_identifier_reference(SPAN, "__name"),
        NONE,
        {
          let mut items = self.builder.vec_with_capacity(2);
          items.push(self.builder.expression_identifier_reference(SPAN, new_name).into());
          items.push(self.builder.expression_string_literal(SPAN, original_name, None).into());
          items
        },
        false,
      ),
    )
  }

  pub fn static_block_keep_name_helper(&self, name: PassedStr) -> ClassElement<'ast> {
    self.builder.class_element_static_block(
      SPAN,
      self.builder.vec1(self.builder.statement_expression(
        SPAN,
        self.builder.expression_call(
          SPAN,
          self.builder.expression_identifier_reference(SPAN, "__name"),
          NONE,
          {
            let mut items = self.builder.vec_with_capacity(2);
            items.push(self.builder.expression_this(SPAN).into());
            items.push(self.builder.expression_string_literal(SPAN, name, None).into());
            items
          },
          false,
        ),
      )),
    )
  }
}
