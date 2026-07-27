//! rolldown's recurring AST constructions, as `new_*` associated functions on
//! extension traits implemented on the oxc AST types they build (`Expression`,
//! `Statement`, ...).
//!
//! Every helper is generic over `B: GetAstBuilder<'ast> + GetAllocator<'ast>` and takes
//! the builder as its **last** argument: oxc's per-type node constructors need
//! [`GetAstBuilder`](oxc::ast::builder::GetAstBuilder), and arena allocation of
//! `Vec`/`Str`/`Box` needs [`GetAllocator`]. oxc's
//! [`AstBuilder`](oxc::ast::builder::AstBuilder) implements both, so it (or any
//! rolldown/oxc context that forwards both traits) can be passed directly.
//!
//! Passing the builder last — rather than as a `self` receiver — is what lets a caller
//! borrow `&mut self` in the same call, e.g.
//! `Expression::new_id_ref_expr(SPAN, self.gen_name(), self)`.
//!
//! See `internal-docs/ast-construction/implementation.md`.

use oxc::{
  allocator::{self, GetAllocator, IntoIn},
  ast::{
    ast::{
      Argument, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator,
      AssignmentTarget, BindingIdentifier, BindingPattern, CallExpression, ClassElement,
      Declaration, ExportDefaultDeclarationKind, ExportSpecifier, Expression, ExpressionStatement,
      FormalParameter, FormalParameterKind, FormalParameters, Function, FunctionBody, FunctionType,
      IdentifierName, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
      ImportNamespaceSpecifier, ImportOrExportKind, MemberExpression, ModuleDeclaration,
      ModuleExportName, NumberBase, ObjectExpression, ObjectPropertyKind, ParenthesizedExpression,
      PropertyKey, PropertyKind, ReturnStatement, SequenceExpression, SimpleAssignmentTarget,
      Statement, StaticMemberExpression, StringLiteral, VariableDeclaration,
      VariableDeclarationKind, VariableDeclarator,
    },
    builder::{GetAstBuilder, NONE},
  },
  span::{GetSpanMut, SPAN, Span},
};
use rolldown_common::{EcmaModuleAstUsage, Interop, MemberExprProp};
use rolldown_utils::ecmascript::is_validate_identifier_name;

/// Options for [`StatementFactoryExt::new_esm_wrapper_stmt`].
pub struct EsmWrapperStmtOptions<'ast, 'data> {
  pub binding_name: &'data str,
  pub esm_fn_expr: Expression<'ast>,
  pub statements: allocator::Vec<'ast, Statement<'ast>>,
  pub profiler_name: Option<&'data str>,
  pub call_kind: EsmWrapperCallKind,
  pub body_kind: EsmWrapperBodyKind,
  pub decl_kind: EsmWrapperDeclKind,
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

#[derive(Clone, Copy)]
pub enum EsmWrapperDeclKind {
  Var,
  HoistedFunction,
}

/// rolldown's recurring `Expression` constructions.
pub trait ExpressionFactoryExt<'ast> {
  /// A reference to `<name>` as an `Expression`, with the name copied into the arena.
  fn new_id_ref_expr<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    span: Span,
    name: &str,
    builder: &B,
  ) -> Expression<'ast> {
    Expression::new_identifier(span, oxc::ast::ast::Str::from_str_in(name, builder), builder)
  }

  /// `() => <expr>`
  fn new_arrow_returning<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    builder: &B,
  ) -> Expression<'ast> {
    let statements = oxc::allocator::Vec::from_value_in(
      Statement::ExpressionStatement(ExpressionStatement::boxed(SPAN, expr, builder)),
      builder,
    );
    Expression::ArrowFunctionExpression(ArrowFunctionExpression::boxed(
      SPAN,
      true,
      false,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::Signature,
        oxc::allocator::Vec::new_in(builder),
        NONE,
        builder,
      ),
      NONE,
      FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(builder), statements, builder),
      builder,
    ))
  }

  /// `<object>.<property>` as an `Expression`.
  fn new_member_access_expr<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    object: &str,
    property: &str,
    builder: &B,
  ) -> Expression<'ast> {
    Expression::from(MemberExpression::new_member_access(object, property, builder))
  }

  /// `<callee>(<arg>)`, optionally annotated `@__PURE__`.
  fn new_call_with_arg<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    callee: Expression<'ast>,
    arg: Expression<'ast>,
    pure: bool,
    builder: &B,
  ) -> Expression<'ast> {
    let mut call_expr =
      CallExpression::new(SPAN, callee, NONE, oxc::allocator::Vec::new_in(builder), false, builder);
    call_expr.pure = pure;
    call_expr.arguments.push(arg.into());
    Expression::CallExpression(call_expr.into_in(builder.allocator()))
  }

  /// `Promise.resolve().then(() => <expr>)`
  fn new_promise_resolve_then<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    builder: &B,
  ) -> Expression<'ast> {
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
              oxc::ast::ast::Str::from_str_in("Promise", builder),
              builder,
            ),
            IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in("resolve", builder), builder),
            false,
            builder,
          )),
          NONE,
          oxc::allocator::Vec::new_in(builder),
          false,
          builder,
        )),
        IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in("then", builder), builder),
        false,
        builder,
      )),
      NONE,
      oxc::allocator::Vec::from_value_in(
        Argument::from(Expression::new_arrow_returning(expr, builder)),
        builder,
      ),
      false,
      builder,
    ))
  }

  /// `None` → `<call_expr>`; `Babel` → `__toESM(<call_expr>)`; `Node` → `__toESM(<call_expr>, 1)`.
  fn new_to_esm_call_with_interop<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    to_esm_fn_name: &str,
    call_expr: Expression<'ast>,
    interop: Option<Interop>,
    builder: &B,
  ) -> Expression<'ast> {
    let arguments = match interop {
      None => return call_expr,
      Some(Interop::Babel) => {
        oxc::allocator::Vec::from_value_in(Argument::from(call_expr), builder)
      }
      Some(Interop::Node) => oxc::allocator::Vec::from_iter_in(
        [
          Argument::from(call_expr),
          Argument::from(Expression::new_numeric_literal(
            SPAN,
            1.0,
            None,
            NumberBase::Decimal,
            builder,
          )),
        ],
        builder,
      ),
    };
    Expression::new_call_expression(
      SPAN,
      Expression::new_identifier(
        SPAN,
        oxc::ast::ast::Str::from_str_in(to_esm_fn_name, builder),
        builder,
      ),
      NONE,
      arguments,
      false,
      builder,
    )
  }

  /// `(<a>, <b>)` — a parenthesized two-element sequence expression.
  fn new_seq_in_parens<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    a: Expression<'ast>,
    b: Expression<'ast>,
    builder: &B,
  ) -> Expression<'ast> {
    let mut expressions = oxc::allocator::Vec::with_capacity_in(2, builder);
    expressions.push(a);
    expressions.push(b);
    Expression::ParenthesizedExpression(ParenthesizedExpression::boxed(
      SPAN,
      Expression::SequenceExpression(SequenceExpression::boxed(SPAN, expressions, builder)),
      builder,
    ))
  }

  /// `<object>.<prop>.<prop>...` — chains member access for each prop, then sets the span.
  fn new_member_expr_or_ident_ref<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    object: Expression<'ast>,
    props: &[MemberExprProp],
    span: Span,
    builder: &B,
  ) -> Expression<'ast> {
    let mut cur = object;
    for prop in props {
      cur = if oxc::syntax::identifier::is_identifier_name(&prop.name) {
        Expression::from(MemberExpression::new_static_member_expression(
          SPAN,
          cur,
          IdentifierName::new(
            prop.span,
            oxc::ast::ast::Str::from_str_in(&prop.name, builder),
            builder,
          ),
          prop.optional,
          builder,
        ))
      } else {
        Expression::from(MemberExpression::new_computed_member_expression(
          SPAN,
          cur,
          Expression::new_string_literal(
            prop.span,
            oxc::ast::ast::Str::from_str_in(&prop.name, builder),
            None,
            builder,
          ),
          prop.optional,
          builder,
        ))
      };
    }
    *cur.span_mut() = span;
    cur
  }

  /// The props of `foo_exports.value.a` is `["value", "a"]`; here convert it to `(void 0).a`.
  #[inline]
  fn new_member_expr_with_void_zero_object<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    props: &[MemberExprProp],
    span: Span,
    builder: &B,
  ) -> Expression<'ast> {
    if props.is_empty() {
      Expression::new_void_0(SPAN, builder)
    } else {
      Expression::new_member_expr_or_ident_ref(
        Expression::new_void_0(SPAN, builder),
        &props[1..],
        span,
        builder,
      )
    }
  }

  /// `<callee>(<original_name as target>, "<original_name>")`, optionally `@__PURE__`.
  fn new_keep_name_call<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    original_name: &str,
    target: Expression<'ast>,
    callee: Expression<'ast>,
    pure: bool,
    builder: &B,
  ) -> Expression<'ast> {
    Expression::new_call_expression_with_pure(
      SPAN,
      callee,
      NONE,
      {
        let mut items = oxc::allocator::Vec::with_capacity_in(2, builder);
        items.push(target.into());
        items.push(
          Expression::new_string_literal(
            SPAN,
            oxc::ast::ast::Str::from_str_in(original_name, builder),
            None,
            builder,
          )
          .into(),
        );
        items
      },
      false,
      pure,
      builder,
    )
  }

  /// `node_mode` ? `__toESM(<expr>, 1)` : `__toESM(<expr>)` (callee `to_esm_fn_expr`, `@__PURE__`).
  fn new_to_esm_wrapper<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    to_esm_fn_expr: Expression<'ast>,
    expr: Expression<'ast>,
    node_mode: bool,
    builder: &B,
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
            builder,
          )),
        ],
        builder,
      )
    } else {
      oxc::allocator::Vec::from_value_in(Argument::from(expr), builder)
    };
    Expression::CallExpression(CallExpression::boxed_with_pure(
      SPAN,
      to_esm_fn_expr,
      NONE,
      args,
      false,
      true,
      builder,
    ))
  }

  /// `<call_expr>.then(() => <return_expr>)`
  fn new_callee_then_call<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    call_expr: Expression<'ast>,
    return_expr: Expression<'ast>,
    builder: &B,
  ) -> Expression<'ast> {
    Expression::CallExpression(CallExpression::boxed(
      SPAN,
      Expression::StaticMemberExpression(StaticMemberExpression::boxed(
        SPAN,
        call_expr,
        IdentifierName::new(SPAN, "then", builder),
        false,
        builder,
      )),
      NONE,
      oxc::allocator::Vec::from_value_in(
        Argument::from(Expression::new_arrow_returning(return_expr, builder)),
        builder,
      ),
      false,
      builder,
    ))
  }
}

impl<'ast> ExpressionFactoryExt<'ast> for Expression<'ast> {}

/// rolldown's recurring `BindingIdentifier` constructions.
pub trait BindingIdentifierFactoryExt<'ast> {
  /// `<name>` as a `BindingIdentifier`, with the name copied into the arena.
  fn new_id<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    span: Span,
    name: &str,
    builder: &B,
  ) -> BindingIdentifier<'ast> {
    BindingIdentifier::new(span, oxc::ast::ast::Str::from_str_in(name, builder), builder)
  }
}

impl<'ast> BindingIdentifierFactoryExt<'ast> for BindingIdentifier<'ast> {}

/// rolldown's recurring `IdentifierName` constructions.
pub trait IdentifierNameFactoryExt<'ast> {
  /// `<name>` as an `IdentifierName`, with the name copied into the arena.
  fn new_id_name<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    span: Span,
    name: &str,
    builder: &B,
  ) -> IdentifierName<'ast> {
    IdentifierName::new(span, oxc::ast::ast::Str::from_str_in(name, builder), builder)
  }
}

impl<'ast> IdentifierNameFactoryExt<'ast> for IdentifierName<'ast> {}

/// rolldown's recurring `MemberExpression` constructions.
pub trait MemberExpressionFactoryExt<'ast> {
  /// `<object>.<property>` as a `MemberExpression`.
  fn new_member_access<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    object: &str,
    property: &str,
    builder: &B,
  ) -> MemberExpression<'ast> {
    MemberExpression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_id_ref_expr(SPAN, object, builder),
      IdentifierName::new_id_name(SPAN, property, builder),
      false,
      builder,
    ))
  }
}

impl<'ast> MemberExpressionFactoryExt<'ast> for MemberExpression<'ast> {}

/// rolldown's recurring `ObjectPropertyKind` constructions.
pub trait ObjectPropertyKindFactoryExt<'ast> {
  /// `<key>: () => <expr>` — a lazy-export object property.
  fn new_lazy_export_property<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    key: &str,
    expr: Expression<'ast>,
    computed: bool,
    builder: &B,
  ) -> ObjectPropertyKind<'ast> {
    ObjectPropertyKind::new_object_property(
      SPAN,
      PropertyKind::Init,
      if computed {
        PropertyKey::from(Expression::new_string_literal(
          SPAN,
          oxc::ast::ast::Str::from_str_in(key, builder),
          None,
          builder,
        ))
      } else {
        PropertyKey::new_static_identifier(
          SPAN,
          oxc::ast::ast::Str::from_str_in(key, builder),
          builder,
        )
      },
      Expression::new_arrow_returning(expr, builder),
      true,
      false,
      computed,
      builder,
    )
  }
}

impl<'ast> ObjectPropertyKindFactoryExt<'ast> for ObjectPropertyKind<'ast> {}

/// rolldown's recurring `ClassElement` constructions.
pub trait ClassElementFactoryExt<'ast> {
  /// `static { <callee>(this, "<name>"); }`
  fn new_static_block_keep_name<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    name: &str,
    callee: Expression<'ast>,
    builder: &B,
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
              let mut items = oxc::allocator::Vec::with_capacity_in(2, builder);
              items.push(Expression::new_this_expression(SPAN, builder).into());
              items.push(
                Expression::new_string_literal(
                  SPAN,
                  oxc::ast::ast::Str::from_str_in(name, builder),
                  None,
                  builder,
                )
                .into(),
              );
              items
            },
            false,
            builder,
          ),
          builder,
        ),
        builder,
      ),
      builder,
    )
  }
}

impl<'ast> ClassElementFactoryExt<'ast> for ClassElement<'ast> {}

/// rolldown's recurring `CallExpression` constructions.
pub trait CallExpressionFactoryExt<'ast> {
  /// `__reExport(<first>, <second>)` (callee provided as `re_export_fn_ref`).
  fn new_re_export_call<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    re_export_fn_ref: Expression<'ast>,
    first_arg: Expression<'ast>,
    second_arg: Expression<'ast>,
    builder: &B,
  ) -> CallExpression<'ast> {
    let args = oxc::allocator::Vec::from_iter_in([first_arg.into(), second_arg.into()], builder);
    CallExpression::new(SPAN, re_export_fn_ref, NONE, args, false, builder)
  }

  /// `<expr>.then(n => n.<property_name>)`
  fn new_then_extract_property<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    property_name: &str,
    builder: &B,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    debug_assert!(is_validate_identifier_name(property_name));
    // `n.<property_name>`
    let member = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", builder),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(property_name, builder), builder),
      false,
      builder,
    ));
    then_with_arrow_callback(expr, member, builder)
  }

  /// `<expr>.then(n => (n.<wrapper_name>(), n.<namespace_name>))`
  fn new_then_call_esm_wrapper_with_namespace<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    wrapper_name: &str,
    namespace_name: &str,
    builder: &B,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let wrapper_member = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", builder),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(wrapper_name, builder), builder),
      false,
      builder,
    ));
    let wrapper_call = Expression::new_call_expression(
      SPAN,
      wrapper_member,
      NONE,
      oxc::allocator::Vec::new_in(builder),
      false,
      builder,
    );
    let namespace_member = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", builder),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(namespace_name, builder), builder),
      false,
      builder,
    ));
    let seq_expr = Expression::new_seq_in_parens(wrapper_call, namespace_member, builder);
    then_with_arrow_callback(expr, seq_expr, builder)
  }

  /// `<expr>.then(n => __toESM(n.<property_name>()))`
  fn new_then_call_cjs_wrapper_with_to_esm<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    property_name: &str,
    to_esm_fn_expr: Expression<'ast>,
    node_mode: bool,
    builder: &B,
  ) -> allocator::Box<'ast, CallExpression<'ast>> {
    let member_expr = Expression::StaticMemberExpression(StaticMemberExpression::boxed(
      SPAN,
      Expression::new_identifier(SPAN, "n", builder),
      IdentifierName::new(SPAN, oxc::ast::ast::Str::from_str_in(property_name, builder), builder),
      false,
      builder,
    ));
    let wrapper_call = Expression::new_call_expression(
      SPAN,
      member_expr,
      NONE,
      oxc::allocator::Vec::new_in(builder),
      false,
      builder,
    );
    let to_esm_call =
      Expression::new_to_esm_wrapper(to_esm_fn_expr, wrapper_call, node_mode, builder);
    then_with_arrow_callback(expr, to_esm_call, builder)
  }
}

impl<'ast> CallExpressionFactoryExt<'ast> for CallExpression<'ast> {}

/// rolldown's recurring `Statement` constructions.
pub trait StatementFactoryExt<'ast> {
  /// `var <name> = <init>;`
  fn new_var_decl<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    name: &str,
    init: Expression<'ast>,
    builder: &B,
  ) -> Statement<'ast> {
    let declarations = oxc::allocator::Vec::from_value_in(
      VariableDeclarator::new(
        SPAN,
        VariableDeclarationKind::Var,
        BindingPattern::new_binding_identifier(
          SPAN,
          oxc::ast::ast::Str::from_str_in(name, builder),
          builder,
        ),
        NONE,
        Some(init),
        false,
        builder,
      ),
      builder,
    );

    Statement::from(Declaration::VariableDeclaration(VariableDeclaration::boxed(
      SPAN,
      VariableDeclarationKind::Var,
      declarations,
      false,
      builder,
    )))
  }

  /// `export default <expr>`
  fn new_export_default_stmt<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    builder: &B,
  ) -> Statement<'ast> {
    Statement::from(ModuleDeclaration::new_export_default_declaration(
      SPAN,
      ExportDefaultDeclarationKind::from(expr),
      builder,
    ))
  }

  /// `module.exports = <expr>`
  fn new_module_exports_stmt<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    expr: Expression<'ast>,
    builder: &B,
  ) -> Statement<'ast> {
    Statement::new_expression_statement(
      SPAN,
      Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::from(SimpleAssignmentTarget::from(
          MemberExpression::new_static_member_expression(
            SPAN,
            Expression::new_identifier(SPAN, "module", builder),
            IdentifierName::new(SPAN, "exports", builder),
            false,
            builder,
          ),
        )),
        expr,
        builder,
      ),
      builder,
    )
  }

  /// `export { <local> as <exported>, ... };`, optionally with a `declaration`.
  fn new_export_named_stmt<'a, T, I, B>(
    declaration: Option<Declaration<'ast>>,
    specifiers: I,
    builder: &B,
  ) -> Statement<'ast>
  where
    T: AsRef<str> + 'a,
    I: Iterator<Item = (&'a T, &'a (T, bool))>,
    B: GetAstBuilder<'ast> + GetAllocator<'ast>,
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
              oxc::ast::ast::Str::from_str_in(local.as_ref(), builder),
              builder,
            ),
            if *legal_ident {
              ModuleExportName::new_identifier_name(
                SPAN,
                oxc::ast::ast::Str::from_str_in(exported.as_ref(), builder),
                builder,
              )
            } else {
              ModuleExportName::new_string_literal(
                SPAN,
                oxc::ast::ast::Str::from_str_in(exported.as_ref(), builder),
                None,
                builder,
              )
            },
            ImportOrExportKind::Value,
            builder,
          )
        }),
        builder,
      ),
      None,
      ImportOrExportKind::Value,
      NONE,
      builder,
    ))
  }

  /// `import * as <as_name> from "<source>";`
  fn new_import_star_stmt<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    source: &str,
    as_name: &str,
    builder: &B,
  ) -> Statement<'ast> {
    let specifiers = oxc::allocator::Vec::from_value_in(
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(ImportNamespaceSpecifier::boxed(
        SPAN,
        BindingIdentifier::new(SPAN, oxc::ast::ast::Str::from_str_in(as_name, builder), builder),
        builder,
      )),
      builder,
    );
    Statement::ImportDeclaration(ImportDeclaration::boxed(
      SPAN,
      Some(specifiers),
      StringLiteral::new(SPAN, oxc::ast::ast::Str::from_str_in(source, builder), None, builder),
      None,
      NONE,
      ImportOrExportKind::Value,
      builder,
    ))
  }

  /// `var <binding_name> = __commonJS(... (exports, module) => { <statements> } ...)`
  #[expect(clippy::too_many_arguments)]
  fn new_commonjs_wrapper_stmt<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    binding_name: &str,
    commonjs_expr: Expression<'ast>,
    statements: allocator::Vec<'ast, Statement<'ast>>,
    ast_usage: EcmaModuleAstUsage,
    profiler_names: bool,
    stable_id: &str,
    is_async: bool,
    builder: &B,
  ) -> Statement<'ast> {
    let mut params = FormalParameters::new(
      SPAN,
      FormalParameterKind::Signature,
      oxc::allocator::Vec::with_capacity_in(1, builder),
      NONE,
      builder,
    );
    let body = FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(builder), statements, builder);
    if ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      params.items.push(FormalParameter::new(
        SPAN,
        oxc::allocator::Vec::new_in(builder),
        BindingPattern::new_binding_identifier(SPAN, "exports", builder),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
        builder,
      ));
    }
    if ast_usage.contains(EcmaModuleAstUsage::ModuleRef) {
      params.items.push(FormalParameter::new(
        SPAN,
        oxc::allocator::Vec::new_in(builder),
        BindingPattern::new_binding_identifier(SPAN, "module", builder),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
        builder,
      ));
    }
    let mut commonjs_call_expr = CallExpression::new_with_pure(
      SPAN,
      commonjs_expr,
      NONE,
      oxc::allocator::Vec::new_in(builder),
      false,
      true,
      builder,
    );
    let mut arrow_expr =
      ArrowFunctionExpression::boxed(SPAN, false, is_async, NONE, params, NONE, body, builder);
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
              oxc::ast::ast::Str::from_str_in(stable_id, builder),
              None,
              builder,
            )),
            Expression::ArrowFunctionExpression(arrow_expr),
            true,
            false,
            false,
            builder,
          ),
          builder,
        ),
        builder,
      );
      commonjs_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      commonjs_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    Statement::new_var_decl(
      binding_name,
      Expression::CallExpression(commonjs_call_expr.into_in(builder.allocator())),
      builder,
    )
  }

  /// `var <binding_name> = __esm(... () => { <statements> } ...)`
  /// or, for order wrappers that must be callable across chunk cycles before assignment:
  /// `function <binding_name>() { return (<binding_name> = __esm(... () => { <statements> } ...))(); }`
  fn new_esm_wrapper_stmt<B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    options: EsmWrapperStmtOptions<'ast, '_>,
    builder: &B,
  ) -> Statement<'ast> {
    let EsmWrapperStmtOptions {
      binding_name,
      esm_fn_expr,
      statements,
      profiler_name,
      call_kind,
      body_kind,
      decl_kind,
    } = options;
    let params = FormalParameters::new(
      SPAN,
      FormalParameterKind::Signature,
      oxc::allocator::Vec::new_in(builder),
      NONE,
      builder,
    );
    let body = FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(builder), statements, builder);
    let mut esm_call_expr = CallExpression::new(
      SPAN,
      esm_fn_expr,
      NONE,
      oxc::allocator::Vec::new_in(builder),
      false,
      builder,
    );
    let mut arrow_expr = ArrowFunctionExpression::boxed(
      SPAN,
      false,
      matches!(body_kind, EsmWrapperBodyKind::Async),
      NONE,
      params,
      NONE,
      body,
      builder,
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
              oxc::ast::ast::Str::from_str_in(stable_id, builder),
              None,
              builder,
            )),
            Expression::ArrowFunctionExpression(arrow_expr),
            false,
            false,
            false,
            builder,
          ),
          builder,
        ),
        builder,
      );
      esm_call_expr.arguments.push(Argument::ObjectExpression(obj_expr));
    } else {
      esm_call_expr.arguments.push(Argument::ArrowFunctionExpression(arrow_expr));
    }
    if matches!(decl_kind, EsmWrapperDeclKind::Var) {
      return Statement::new_var_decl(
        binding_name,
        Expression::CallExpression(esm_call_expr.into_in(builder.allocator())),
        builder,
      );
    }
    let assignment_expr = Expression::AssignmentExpression(AssignmentExpression::boxed(
      SPAN,
      AssignmentOperator::Assign,
      AssignmentTarget::AssignmentTargetIdentifier(IdentifierReference::boxed(
        SPAN,
        oxc::ast::ast::Str::from_str_in(binding_name, builder),
        builder,
      )),
      Expression::CallExpression(esm_call_expr.into_in(builder.allocator())),
      builder,
    ));
    let call_expr = Expression::new_call_expression(
      SPAN,
      assignment_expr,
      NONE,
      oxc::allocator::Vec::new_in(builder),
      false,
      builder,
    );
    Statement::FunctionDeclaration(Function::boxed(
      SPAN,
      FunctionType::FunctionDeclaration,
      Some(BindingIdentifier::new(
        SPAN,
        oxc::ast::ast::Str::from_str_in(binding_name, builder),
        builder,
      )),
      false,
      false,
      false,
      NONE,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::Signature,
        oxc::allocator::Vec::new_in(builder),
        NONE,
        builder,
      ),
      NONE,
      Some(FunctionBody::new(
        SPAN,
        oxc::allocator::Vec::new_in(builder),
        oxc::allocator::Vec::from_value_in(
          Statement::ReturnStatement(ReturnStatement::boxed(SPAN, Some(call_expr), builder)),
          builder,
        ),
        builder,
      )),
      builder,
    ))
  }
}

impl<'ast> StatementFactoryExt<'ast> for Statement<'ast> {}

/// `<expr>.then(n => <return_expr>)`
fn then_with_arrow_callback<'ast, B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
  expr: Expression<'ast>,
  return_expr: Expression<'ast>,
  builder: &B,
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
          oxc::allocator::Vec::new_in(builder),
          BindingPattern::new_binding_identifier(
            SPAN,
            oxc::ast::ast::Str::from_str_in("n", builder),
            builder,
          ),
          NONE,
          NONE,
          false,
          None,
          false,
          false,
          builder,
        ),
        builder,
      ),
      NONE,
      builder,
    ),
    NONE,
    FunctionBody::new(
      SPAN,
      oxc::allocator::Vec::new_in(builder),
      oxc::allocator::Vec::from_value_in(
        Statement::ExpressionStatement(ExpressionStatement::boxed(SPAN, return_expr, builder)),
        builder,
      ),
      builder,
    ),
    builder,
  );
  let callee = StaticMemberExpression::boxed(
    SPAN,
    expr,
    IdentifierName::new(SPAN, "then", builder),
    false,
    builder,
  );
  CallExpression::boxed(
    SPAN,
    Expression::StaticMemberExpression(callee),
    NONE,
    oxc::allocator::Vec::from_value_in(
      Expression::ArrowFunctionExpression(arrow_fn).into(),
      builder,
    ),
    false,
    builder,
  )
}
