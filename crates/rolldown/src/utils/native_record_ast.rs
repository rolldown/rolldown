//! Direct arena construction of a native record's `cells` and `temporal` registration objects.
//!
//! These build the span-free AST `parse_injected_expression` would produce from the equivalent
//! source text (`{name:[()=>local,_=>local=_],…}` and `{name:()=>flag,…}`) without formatting
//! and reparsing it for every record. A key is an identifier when the name is an ASCII
//! identifier name other than `__proto__`, and a string literal otherwise; both define the same
//! own property. `__proto__` is a computed key: a non-computed `__proto__` key in an object
//! literal sets the prototype instead of defining an own property. A setter's completion value is
//! ignored by its only caller.

use oxc::{
  allocator::Vec as ArenaVec,
  ast::{
    ast::{self, Expression},
    builder::AstBuilder,
  },
  span::SPAN,
};

/// The registration a native record publishes for one export it declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'n> {
  /// `0`: any update to the record requires a reload.
  Reload,
  /// `[()=>local]`
  Get(&'n str),
  /// `[()=>local,_=>local=_]`
  GetSet(&'n str),
  /// `[()=>namespace["name"]]`
  Facade(&'n str),
}

fn string<'ast>(builder: &AstBuilder<'ast>, value: &str) -> Expression<'ast> {
  Expression::new_string_literal(SPAN, ast::Str::from_str_in(value, builder), None, builder)
}

fn identifier<'ast>(builder: &AstBuilder<'ast>, name: &str) -> Expression<'ast> {
  Expression::new_identifier(SPAN, ast::Ident::from_str_in(name, builder), builder)
}

fn arrow<'ast>(
  builder: &AstBuilder<'ast>,
  params: ArenaVec<'ast, ast::FormalParameter<'ast>>,
  body: ast::ArrowFunctionBody<'ast>,
) -> Expression<'ast> {
  Expression::new_arrow_function_expression(
    SPAN,
    false,
    None,
    ast::FormalParameters::boxed(
      SPAN,
      ast::FormalParameterKind::ArrowFormalParameters,
      params,
      None,
      builder,
    ),
    None,
    body,
    builder,
  )
}

/// `()=>value`
fn getter<'ast>(builder: &AstBuilder<'ast>, value: Expression<'ast>) -> Expression<'ast> {
  arrow(builder, ArenaVec::new_in(builder), ast::ArrowFunctionBody::from(value))
}

/// `_=>local=_`
fn setter<'ast>(builder: &AstBuilder<'ast>, local: &str) -> Expression<'ast> {
  let param = ast::FormalParameter::new(
    SPAN,
    ArenaVec::new_in(builder),
    ast::BindingPattern::new_binding_identifier(SPAN, "_", builder),
    None,
    None,
    false,
    None,
    false,
    false,
    builder,
  );
  let assignment = Expression::new_assignment_expression(
    SPAN,
    ast::AssignmentOperator::Assign,
    ast::AssignmentTarget::new_assignment_target_identifier(
      SPAN,
      ast::Ident::from_str_in(local, builder),
      builder,
    ),
    identifier(builder, "_"),
    builder,
  );
  arrow(builder, ArenaVec::from_iter_in([param], builder), ast::ArrowFunctionBody::from(assignment))
}

fn array<'ast>(
  builder: &AstBuilder<'ast>,
  elements: impl IntoIterator<Item = Expression<'ast>>,
) -> Expression<'ast> {
  Expression::new_array_expression(
    SPAN,
    ArenaVec::from_iter_in(elements.into_iter().map(ast::ArrayExpressionElement::from), builder),
    builder,
  )
}

/// Whether `name` prints as an identifier key: ASCII only, so every engine that runs the record
/// reads it the same way, and never `__proto__`.
fn identifier_key(name: &str) -> bool {
  let mut bytes = name.bytes();
  bytes.next().is_some_and(|first| first.is_ascii_alphabetic() || first == b'_' || first == b'$')
    && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$')
    && name != "__proto__"
}

fn object<'ast, 'n>(
  builder: &AstBuilder<'ast>,
  properties: impl IntoIterator<Item = (&'n str, Expression<'ast>)>,
) -> Expression<'ast> {
  let properties = properties.into_iter().map(|(name, value)| {
    let computed = name == "__proto__";
    let key = if computed {
      ast::PropertyKey::from(string(builder, name))
    } else if identifier_key(name) {
      ast::PropertyKey::new_static_identifier(SPAN, ast::Ident::from_str_in(name, builder), builder)
    } else {
      ast::PropertyKey::new_string_literal(SPAN, ast::Str::from_str_in(name, builder), None, builder)
    };
    ast::ObjectPropertyKind::new_object_property(
      SPAN,
      ast::PropertyKind::Init,
      key,
      value,
      false,
      false,
      computed,
      builder,
    )
  });
  Expression::new_object_expression(SPAN, ArenaVec::from_iter_in(properties, builder), builder)
}

fn cell<'ast>(builder: &AstBuilder<'ast>, name: &str, cell: Cell<'_>) -> Expression<'ast> {
  match cell {
    Cell::Reload => Expression::new_numeric_literal(
      SPAN,
      0.0,
      Some(ast::Str::from("0")),
      ast::NumberBase::Decimal,
      builder,
    ),
    Cell::Get(local) => array(builder, [getter(builder, identifier(builder, local))]),
    Cell::GetSet(local) => {
      array(builder, [getter(builder, identifier(builder, local)), setter(builder, local)])
    }
    Cell::Facade(namespace) => {
      let member = Expression::new_computed_member_expression(
        SPAN,
        identifier(builder, namespace),
        string(builder, name),
        false,
        builder,
      );
      array(builder, [getter(builder, member)])
    }
  }
}

/// `{name:cell,…}` in the given order.
pub fn cells<'ast, 'n>(
  builder: &AstBuilder<'ast>,
  entries: impl IntoIterator<Item = (&'n str, Cell<'n>)>,
) -> Expression<'ast> {
  object(
    builder,
    entries.into_iter().map(|(name, value)| (name, cell(builder, name, value))),
  )
}

/// `{name:()=>flag,…}` in the given order.
pub fn temporal<'ast, 'n>(
  builder: &AstBuilder<'ast>,
  entries: impl IntoIterator<Item = (&'n str, &'n str)>,
) -> Expression<'ast> {
  object(
    builder,
    entries.into_iter().map(|(name, flag)| (name, getter(builder, identifier(builder, flag)))),
  )
}
