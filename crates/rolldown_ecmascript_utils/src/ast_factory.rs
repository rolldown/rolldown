use oxc::{
  allocator::{self, Allocator, GetAllocator, IntoIn},
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, ArrowFunctionExpression, AssignmentOperator, AssignmentTarget, BindingIdentifier,
      BindingPattern, CallExpression, ClassElement, Declaration, ExportDefaultDeclarationKind,
      ExportSpecifier, Expression, ExpressionStatement, FormalParameter, FormalParameterKind,
      FormalParameters, FunctionBody, IdentifierName, ImportDeclaration,
      ImportDeclarationSpecifier, ImportNamespaceSpecifier, ImportOrExportKind, MemberExpression,
      ModuleDeclaration, ModuleExportName, NumberBase, ObjectExpression, ObjectPropertyKind,
      ParenthesizedExpression, PropertyKey, PropertyKind, SequenceExpression,
      SimpleAssignmentTarget, Statement, StaticMemberExpression, StringLiteral,
      VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
    },
    builder::GetAstBuilder,
  },
  span::{GetSpanMut, SPAN, Span},
};
use rolldown_common::{EcmaModuleAstUsage, Interop, MemberExprProp};
use rolldown_utils::ecmascript::is_validate_identifier_name;

/// Rolldown's newtype wrapper around oxc's [`AstBuilder`].
///
/// It implements oxc's [`GetAstBuilder`] and [`GetAllocator`] traits, so it can be
/// passed directly to oxc's per-type node constructors (`Expression::new_*`,
/// `Foo::boxed`, `oxc::allocator::Vec::new_in`, ...). rolldown's own recurring
/// constructions are added as inherent `make_*` methods. Routing all construction
/// through this single type lets rolldown absorb future oxc construction-API changes
/// at one point.
///
/// See `internal-docs/ast-construction/implementation.md`.
#[derive(Clone, Copy)]
pub struct AstFactory<'ast>(AstBuilder<'ast>);

/// Options for [`AstFactory::make_esm_wrapper_stmt`], grouping the wrapper's binding, body and
/// call shape so new emission modes can be added without growing the argument list.
pub struct EsmWrapperStmtOptions<'ast, 'data> {
  pub binding_name: &'data str,
  pub esm_fn_expr: Expression<'ast>,
  pub statements: allocator::Vec<'ast, Statement<'ast>>,
  /// `Some(stable_id)` wraps the closure in a profiler-named object argument.
  pub profiler_name: Option<&'data str>,
  pub call_kind: EsmWrapperCallKind,
  pub body_kind: EsmWrapperBodyKind,
}

#[derive(Clone, Copy)]
pub enum EsmWrapperCallKind {
  Plain,
  Pife,
}

#[derive(Clone, Copy)]
pub enum EsmWrapperBodyKind {
  Sync,
  Async,
}

impl<'ast> AstFactory<'ast> {
  pub fn new(allocator: &'ast Allocator) -> Self {
    Self(AstBuilder::new(allocator))
  }

  /// `<name>` as a `BindingIdentifier`, with the name copied into the arena.
  pub fn make_id(&self, span: Span, name: &str) -> BindingIdentifier<'ast> {
    BindingIdentifier::new(span, oxc::ast::ast::Str::from_str_in(name, self), self)
  }

  /// A reference to `<name>` as an `Expression`, with the name copied into the arena.
  pub fn make_id_ref_expr(&self, span: Span, name: &str) -> Expression<'ast> {
    Expression::new_identifier(span, oxc::ast::ast::Str::from_str_in(name, self), self)
  }

  /// `<name>` as an `IdentifierName`, with the name copied into the arena.
  pub fn make_id_name(&self, span: Span, name: &str) -> IdentifierName<'ast> {
    IdentifierName::new(span, oxc::ast::ast::Str::from_str_in(name, self), self)
  }

  /// `var <name> = <init>;`
  pub fn make_var_decl(&self, name: &str, init: Expression<'ast>) -> Statement<'ast> {
    let declarations = oxc::allocator::Vec::from_value_in(
      VariableDeclarator::new(
        SPAN,
        VariableDeclarationKind::Var,
        BindingPattern::new_binding_identifier(
          SPAN,
          oxc::ast::ast::Str::from_str_in(name, self),
          self,
        ),
        NONE,
        Some(init),
        false,
        self,
      ),
      self,
    );

    Statement::from(Declaration::VariableDeclaration(VariableDeclaration::boxed(
      SPAN,
      VariableDeclarationKind::Var,
      declarations,
      false,
      self,
    )))
  }

  /// `export default <expr>`
  pub fn make_export_default_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    Statement::from(ModuleDeclaration::new_export_default_declaration(
      SPAN,
      ExportDefaultDeclarationKind::from(expr),
      self,
    ))
  }

  /// `module.exports = <expr>`
  pub fn make_module_exports_stmt(&self, expr: Expression<'ast>) -> Statement<'ast> {
    Statement::new_expression_statement(
      SPAN,
      Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::from(SimpleAssignmentTarget::from(
          MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, "module", self),
            IdentifierName::new(SPAN, "exports", self),
            false,
            self,
          ),
        )),
        expr,
        self,
      ),
      self,
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
    Statement::from(ModuleDeclaration::new_export_named_declaration(
      SPAN,
      declaration,
      oxc::allocator::Vec::from_iter_in(
        specifiers.into_iter().map(|(local, (exported, legal_ident))| {
          ExportSpecifier::new(
            SPAN,
            ModuleExportName::new_identifier_reference(
              SPAN,
              oxc::ast::ast::Str::from_str_in(local.as_ref(), self),
              self,
            ),
            if *legal_ident {
              ModuleExportName::new_identifier_name(
                SPAN,
                oxc::ast::ast::Str::from_str_in(exported.as_ref(), self),
                self,
              )
            } else {
              ModuleExportName::new_string_literal(
                SPAN,
                oxc::ast::ast::Str::from_str_in(exported.as_ref(), self),
                None,
                self,
              )
            },
            ImportOrExportKind::Value,
            self,
          )
        }),
        self,
      ),
      None,
      ImportOrExportKind::Value,
      NONE,
      self,
    ))
  }

  /// `() => <expr>`
  pub fn make_arrow_returning(&self, expr: Expression<'ast>) -> Expression<'ast> {
    let statements = oxc::allocator::Vec::from_value_in(
      Statement::ExpressionStatement(ExpressionStatement::boxed(SPAN, expr, self)),
      self,
    );
    Expression::ArrowFunctionExpression(ArrowFunctionExpression::boxed(
      SPAN,
      true,
      false,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::Signature,
        oxc::allocator::Vec::new_in(self),
        NONE,
        self,
      ),
      NONE,
      FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(self), statements, self),
      self,
    ))
  }

  /// `<object>.<property>` as a `MemberExpression`.
  pub fn make_member_access(&self, object: &str, property: &str) -> MemberExpression<'ast> {
    MemberExpression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      self.make_id_ref_expr(SPAN, object),
      self.make_id_name(SPAN, property),
      false,
      self,
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
    let mut call_expr =
      CallExpression::new(SPAN, callee, NONE, oxc::allocator::Vec::new_in(self), false, self);
    call_expr.pure = pure;
    call_expr.arguments.push(arg.into());
    Expression::CallExpression(call_expr.into_in(self.allocator()))
  }

  /// `Promise.resolve().then(() => <expr>)`
  pub fn make_promise_resolve_then(&self, expr: Expression<'ast>) -> Expression<'ast> {
    Expression::CallExpression(CallExpression::boxed(
      SPAN,
      Expression::StaticMemberExpression(StaticMemberExpression::boxed(
        SPAN,
        Expression::CallExpression(CallExpression::boxed(
          SPAN,
          Expression::StaticMemberExpression(StaticMemberExpression::boxed(
            SPAN,
            Expression::new_identifier(
              SPAN,
              oxc::ast::ast::Str::from_str_in("Promise", self),
              self,
            ),
            IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in("resolve", self), self),
            false,
            self,
          )),
          NONE,
          oxc::allocator::Vec::new_in(self),
          false,
          self,
        )),
        IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in("then", self), self),
        false,
        self,
      )),
      NONE,
      oxc::allocator::Vec::from_value_in(Argument::from(self.make_arrow_returning(expr)), self),
      false,
      self,
    ))
  }

  /// `<key>: () => <expr>` — a lazy-export object property.
  pub fn make_lazy_export_property(
    &self,
    key: &str,
    expr: Expression<'ast>,
    computed: bool,
  ) -> ObjectPropertyKind<'ast> {
    ObjectPropertyKind::new_object_property(
      SPAN,
      PropertyKind::Init,
      if computed {
        PropertyKey::from(Expression::new_string_literal(
          SPAN,
          oxc::ast::ast::Str::from_str_in(key, self),
          None,
          self,
        ))
      } else {
        PropertyKey::new_static_identifier(SPAN, oxc::ast::ast::Str::from_str_in(key, self), self)
      },
      self.make_arrow_returning(expr),
      true,
      false,
      computed,
      self,
    )
  }

  /// `import * as <as_name> from "<source>";`
  pub fn make_import_star_stmt(&self, source: &str, as_name: &str) -> Statement<'ast> {
    let specifiers = oxc::allocator::Vec::from_value_in(
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(ImportNamespaceSpecifier::boxed(
        SPAN,
        BindingIdentifier::new(SPAN, oxc::ast::ast::Str::from_str_in(as_name, self), self),
        self,
      )),
      self,
    );
    Statement::ImportDeclaration(ImportDeclaration::boxed(
      SPAN,
      Some(specifiers),
      StringLiteral::new(SPAN, oxc::ast::ast::Str::from_str_in(source, self), None, self),
      None,
      NONE,
      ImportOrExportKind::Value,
      self,
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
      Some(Interop::Babel) => oxc::allocator::Vec::from_value_in(Argument::from(call_expr), self),
      Some(Interop::Node) => oxc::allocator::Vec::from_iter_in(
        [
          Argument::from(call_expr),
          Argument::from(Expression::new_numeric_literal(
            SPAN,
            1.0,
            None,
            NumberBase::Decimal,
            self,
          )),
        ],
        self,
      ),
    };
    Expression::new_call_expression(
      SPAN,
      Expression::new_identifier(SPAN, oxc::ast::ast::Str::from_str_in(to_esm_fn_name, self), self),
      NONE,
      arguments,
      false,
      self,
    )
  }

  /// `(<a>, <b>)` — a parenthesized two-element sequence expression.
  pub fn make_seq_in_parens(&self, a: Expression<'ast>, b: Expression<'ast>) -> Expression<'ast> {
    let mut expressions = oxc::allocator::Vec::with_capacity_in(2, self);
    expressions.push(a);
    expressions.push(b);
    Expression::ParenthesizedExpression(ParenthesizedExpression::boxed(
      SPAN,
      Expression::SequenceExpression(SequenceExpression::boxed(SPAN, expressions, self)),
      self,
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
        Expression::from(MemberExpression::new_static_member_expression(
          SPAN,
          cur,
          IdentifierName::new(prop.span, oxc::ast::ast::Str::from_str_in(&prop.name, self), self),
          prop.optional,
          self,
        ))
      } else {
        Expression::from(MemberExpression::new_computed_member_expression(
          SPAN,
          cur,
          Expression::new_string_literal(
            prop.span,
            oxc::ast::ast::Str::from_str_in(&prop.name, self),
            None,
            self,
          ),
          prop.optional,
          self,
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
      Expression::new_void_0(SPAN, self)
    } else {
      self.make_member_expr_or_ident_ref(Expression::new_void_0(SPAN, self), &props[1..], span)
    }
  }

  /// `__reExport(<first>, <second>)` (callee provided as `re_export_fn_ref`).
  pub fn make_re_export_call(
    &self,
    re_export_fn_ref: Expression<'ast>,
    first_arg: Expression<'ast>,
    second_arg: Expression<'ast>,
  ) -> CallExpression<'ast> {
    let args = oxc::allocator::Vec::from_iter_in([first_arg.into(), second_arg.into()], self);
    CallExpression::new(SPAN, re_export_fn_ref, NONE, args, false, self)
  }

  /// `<callee>(<original_name as target>, "<original_name>")`, optionally `@__PURE__`.
  pub fn make_keep_name_call(
    &self,
    original_name: &str,
    target: Expression<'ast>,
    callee: Expression<'ast>,
    pure: bool,
  ) -> Expression<'ast> {
    Expression::new_call_expression_with_pure(
      SPAN,
      callee,
      NONE,
      {
        let mut items = oxc::allocator::Vec::with_capacity_in(2, self);
        items.push(target.into());
        items.push(
          Expression::new_string_literal(
            SPAN,
            oxc::ast::ast::Str::from_str_in(original_name, self),
            None,
            self,
          )
          .into(),
        );
        items
      },
      false,
      pure,
      self,
    )
  }

  /// `static { <callee>(this, "<name>"); }`
  pub fn make_static_block_keep_name(
    &self,
    name: &str,
    callee: Expression<'ast>,
  ) -> ClassElement<'ast> {
    ClassElement::new_static_block(
      SPAN,
      oxc::allocator::Vec::from_value_in(
        Statement::new_expression_statement(
          SPAN,
          Expression::new_call_expression(
            SPAN,
            callee,
            NONE,
            {
              let mut items = oxc::allocator::Vec::with_capacity_in(2, self);
              items.push(Expression::new_this_expression(SPAN, self).into());
              items.push(
                Expression::new_string_literal(
                  SPAN,
                  oxc::ast::ast::Str::from_str_in(name, self),
                  None,
                  self,
                )
                .into(),
              );
              items
            },
            false,
            self,
          ),
          self,
        ),
        self,
      ),
      self,
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
      oxc::allocator::Vec::from_iter_in(
        [
          Argument::from(expr),
          Argument::from(Expression::new_numeric_literal(
            SPAN,
            1.0,
            None,
            NumberBase::Decimal,
            self,
          )),
        ],
        self,
      )
    } else {
      oxc::allocator::Vec::from_value_in(Argument::from(expr), self)
    };
    Expression::CallExpression(CallExpression::boxed_with_pure(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
      true,
      self,
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
    let mut params = FormalParameters::new(
      SPAN,
      FormalParameterKind::Signature,
      oxc::allocator::Vec::with_capacity_in(1, self),
      NONE,
      self,
    );
    let body = FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(self), statements, self);
    if ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      params.items.push(FormalParameter::new(
        SPAN,
        oxc::allocator::Vec::new_in(self),
        BindingPattern::new_binding_identifier(SPAN, "exports", self),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
        self,
      ));
    }
    if ast_usage.contains(EcmaModuleAstUsage::ModuleRef) {
      params.items.push(FormalParameter::new(
        SPAN,
        oxc::allocator::Vec::new_in(self),
        BindingPattern::new_binding_identifier(SPAN, "module", self),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
        self,
      ));
    }
    let mut commonjs_call_expr = CallExpression::new_with_pure(
      SPAN,
      commonjs_expr,
      NONE,
      oxc::allocator::Vec::new_in(self),
      false,
      true,
      self,
    );
    let mut arrow_expr =
      ArrowFunctionExpression::boxed(SPAN, false, is_async, NONE, params, NONE, body, self);
    arrow_expr.pife = true;
    if profiler_names {
      let obj_expr = ObjectExpression::boxed(
        SPAN,
        oxc::allocator::Vec::from_value_in(
          ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::from(Expression::new_string_literal(
              SPAN,
              oxc::ast::ast::Str::from_str_in(stable_id, self),
              None,
              self,
            )),
            Expression::ArrowFunctionExpression(arrow_expr),
            true,
            false,
            false,
            self,
          ),
          self,
        ),
        self,
      );
      commonjs_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      commonjs_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    self.make_var_decl(
      binding_name,
      Expression::CallExpression(commonjs_call_expr.into_in(self.allocator())),
    )
  }

  /// `var <binding_name> = __esm(... () => { <statements> } ...)`
  pub fn make_esm_wrapper_stmt(&self, options: EsmWrapperStmtOptions<'ast, '_>) -> Statement<'ast> {
    let EsmWrapperStmtOptions {
      binding_name,
      esm_fn_expr,
      statements,
      profiler_name,
      call_kind,
      body_kind,
    } = options;
    let params = FormalParameters::new(
      SPAN,
      FormalParameterKind::Signature,
      oxc::allocator::Vec::new_in(self),
      NONE,
      self,
    );
    let body = FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(self), statements, self);
    let mut esm_call_expr =
      CallExpression::new(SPAN, esm_fn_expr, NONE, oxc::allocator::Vec::new_in(self), false, self);
    let mut arrow_expr = ArrowFunctionExpression::boxed(
      SPAN,
      false,
      matches!(body_kind, EsmWrapperBodyKind::Async),
      NONE,
      params,
      NONE,
      body,
      self,
    );
    arrow_expr.pife = matches!(call_kind, EsmWrapperCallKind::Pife);
    if let Some(stable_id) = profiler_name {
      let obj_expr = ObjectExpression::boxed(
        SPAN,
        oxc::allocator::Vec::from_value_in(
          ObjectPropertyKind::new_object_property(
            SPAN,
            PropertyKind::Init,
            PropertyKey::from(Expression::new_string_literal(
              SPAN,
              oxc::ast::ast::Str::from_str_in(stable_id, self),
              None,
              self,
            )),
            Expression::ArrowFunctionExpression(arrow_expr),
            false,
            false,
            false,
            self,
          ),
          self,
        ),
        self,
      );
      esm_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      esm_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    self.make_var_decl(
      binding_name,
      Expression::CallExpression(esm_call_expr.into_in(self.allocator())),
    )
  }

  /// `n => n.<property_name>`
  fn arrow_function_extract_property(
    &self,
    property_name: &str,
  ) -> allocator::Box<'ast, ArrowFunctionExpression<'ast>> {
    debug_assert!(is_validate_identifier_name(property_name));
    ArrowFunctionExpression::boxed(
      SPAN,
      true,
      false,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        oxc::allocator::Vec::from_value_in(
          FormalParameter::new(
            SPAN,
            oxc::allocator::Vec::new_in(self),
            BindingPattern::new_binding_identifier(
              SPAN,
              oxc::ast::ast::Str::from_str_in("n", self),
              self,
            ),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
            self,
          ),
          self,
        ),
        NONE,
        self,
      ),
      NONE,
      FunctionBody::new(
        SPAN,
        oxc::allocator::Vec::new_in(self),
        oxc::allocator::Vec::from_value_in(
          Statement::ExpressionStatement(ExpressionStatement::boxed(
            SPAN,
            Expression::StaticMemberExpression(StaticMemberExpression::boxed(
              SPAN,
              Expression::new_identifier(SPAN, "n", self),
              IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(property_name, self), self),
              false,
              self,
            )),
            self,
          )),
          self,
        ),
        self,
      ),
      self,
    )
  }

  /// `<expr>.then(n => <return_expr>)`
  fn then_with_arrow_callback(
    &self,
    expr: Expression<'ast>,
    return_expr: Expression<'ast>,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let arrow_fn = ArrowFunctionExpression::boxed(
      SPAN,
      true,
      false,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        oxc::allocator::Vec::from_value_in(
          FormalParameter::new(
            SPAN,
            oxc::allocator::Vec::new_in(self),
            BindingPattern::new_binding_identifier(
              SPAN,
              oxc::ast::ast::Str::from_str_in("n", self),
              self,
            ),
            NONE,
            NONE,
            false,
            None,
            false,
            false,
            self,
          ),
          self,
        ),
        NONE,
        self,
      ),
      NONE,
      FunctionBody::new(
        SPAN,
        oxc::allocator::Vec::new_in(self),
        oxc::allocator::Vec::from_value_in(
          Statement::ExpressionStatement(ExpressionStatement::boxed(SPAN, return_expr, self)),
          self,
        ),
        self,
      ),
      self,
    );
    let callee = StaticMemberExpression::boxed(
      SPAN,
      expr,
      IdentifierName::new(SPAN, "then", self),
      false,
      self,
    );
    CallExpression::boxed(
      SPAN,
      Expression::StaticMemberExpression(callee),
      NONE,
      oxc::allocator::Vec::from_value_in(
        Expression::ArrowFunctionExpression(arrow_fn).into(),
        self,
      ),
      false,
      self,
    )
  }

  /// `<expr>.then(n => n.<property_name>)`
  pub fn make_then_extract_property(
    &self,
    expr: Expression<'ast>,
    property_name: &str,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let callee = StaticMemberExpression::boxed(
      SPAN,
      expr,
      IdentifierName::new(SPAN, "then", self),
      false,
      self,
    );
    CallExpression::boxed(
      SPAN,
      Expression::StaticMemberExpression(callee),
      NONE,
      oxc::allocator::Vec::from_value_in(
        Expression::ArrowFunctionExpression(self.arrow_function_extract_property(property_name))
          .into(),
        self,
      ),
      false,
      self,
    )
  }

  /// `<expr>.then(n => (n.<wrapper_name>(), n.<namespace_name>))`
  pub fn make_then_call_esm_wrapper_with_namespace(
    &self,
    expr: Expression<'ast>,
    wrapper_name: &str,
    namespace_name: &str,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let wrapper_member = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", self),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(wrapper_name, self), self),
      false,
      self,
    ));
    let wrapper_call = Expression::new_call_expression(
      SPAN,
      wrapper_member,
      NONE,
      oxc::allocator::Vec::new_in(self),
      false,
      self,
    );
    let namespace_member = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", self),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(namespace_name, self), self),
      false,
      self,
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
    let member_expr = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", self),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(property_name, self), self),
      false,
      self,
    ));
    let wrapper_call = Expression::new_call_expression(
      SPAN,
      member_expr,
      NONE,
      oxc::allocator::Vec::new_in(self),
      false,
      self,
    );
    let to_esm_call = self.make_to_esm_wrapper(to_esm_fn_expr, wrapper_call, node_mode);
    self.then_with_arrow_callback(expr, to_esm_call)
  }

  /// `<call_expr>.then(() => <return_expr>)`
  pub fn make_callee_then_call(
    &self,
    call_expr: Expression<'ast>,
    return_expr: Expression<'ast>,
  ) -> Expression<'ast> {
    Expression::CallExpression(CallExpression::boxed(
      SPAN,
      Expression::StaticMemberExpression(StaticMemberExpression::boxed(
        SPAN,
        call_expr,
        IdentifierName::new(SPAN, "then", self),
        false,
        self,
      )),
      NONE,
      oxc::allocator::Vec::from_value_in(
        Argument::from(self.make_arrow_returning(return_expr)),
        self,
      ),
      false,
      self,
    ))
  }
}

impl<'ast> GetAllocator<'ast> for AstFactory<'ast> {
  #[inline]
  fn allocator(&self) -> &'ast Allocator {
    self.0.allocator()
  }
}

impl<'ast> GetAstBuilder<'ast> for AstFactory<'ast> {
  type Builder = AstBuilder<'ast>;

  #[inline]
  fn builder(&self) -> &AstBuilder<'ast> {
    &self.0
  }
}
