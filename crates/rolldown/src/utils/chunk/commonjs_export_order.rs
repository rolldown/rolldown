//! The order in which Node reads the export names of a CommonJS module. See
//! internal-docs/code-splitting/implementation.md ("Shared CommonJS property reads").

use oxc::ast::ast::{
  Argument, AssignmentExpression, AssignmentTarget, CallExpression, Expression, MemberExpression,
  ObjectPropertyKind, Program, PropertyKey, PropertyKind, Statement,
};
use oxc::ast_visit::{VisitJs, walk_js};
use oxc::span::{GetSpan, Span};
use oxc_str::CompactStr;
use rustc_hash::FxHashMap;

/// The source positions of the export names of a CommonJS module.
///
/// When an ES module imports a CommonJS module, the CommonJS lexer of Node finds the export names
/// in the source text. Node then reads each name one time, in the order of the first place where
/// the lexer finds the name. A later place does not move the name. This type records these places:
/// - `exports.name`, `exports['name']`, `module.exports.name` or `module.exports['name']` before
///   the character `=`. The lexer finds such a name by the `=` after it, at any depth: in an
///   assignment, in a comparison with `==` or `===`, and in a default value of a destructuring
///   assignment. The lexer does not resolve scopes, so a local variable `exports` also counts. A
///   compound assignment, for example `+=`, does not count.
/// - A call `Object.defineProperty(exports, 'name', descriptor)`, also with `module.exports`,
///   whose descriptor has a getter of a form that the lexer finds. See [`is_found_getter`].
///
/// The parser removes parentheses from the AST, but the lexer reads the source text. Thus each
/// place has a source text check. A place whose text the rule does not identify is an unsure
/// place: the lexer can find the name there, or not. For example, `(exports).name = 0`, an escape
/// in a name, or a character that is not ASCII next to the name gives an unsure place. Each other
/// call of `Object.defineProperty` on `exports` with a string name is also an unsure place.
///
/// The position of a name is not known in these conditions:
/// - The name has no place, for example because it comes from
///   `module.exports = require('./other')`.
/// - The first place of the name is an unsure place.
/// - The module assigns `module.exports` or `exports`. This applies to all names.
#[derive(Debug, Default)]
pub struct CommonJsExportOrder {
  first_places: FxHashMap<CompactStr, Place>,
  replaces_exports: bool,
}

/// A place of an export name, with the start of its source span.
#[derive(Debug, Clone, Copy)]
enum Place {
  /// The lexer finds the name at this place.
  Found(u32),
  /// The lexer can find the name at this place, or not.
  Unsure(u32),
}

impl Place {
  fn start(self) -> u32 {
    match self {
      Self::Found(start) | Self::Unsure(start) => start,
    }
  }
}

impl CommonJsExportOrder {
  pub fn new(program: &Program<'_>) -> Self {
    let mut scanner = Scanner { source: program.source_text, order: Self::default() };
    scanner.visit_program(program);
    scanner.order
  }

  /// The source position of the first place of `name`, if the lexer finds the name at that place.
  /// Node reads the names in the order of these positions.
  pub fn position(&self, name: &str) -> Option<u32> {
    if self.replaces_exports {
      return None;
    }
    match self.first_places.get(name)? {
      Place::Found(start) => Some(*start),
      Place::Unsure(_) => None,
    }
  }

  fn record(&mut self, name: &str, place: Place) {
    match self.first_places.get_mut(name) {
      Some(first) => {
        if place.start() < first.start() {
          *first = place;
        }
      }
      None => {
        self.first_places.insert(name.into(), place);
      }
    }
  }
}

struct Scanner<'s> {
  source: &'s str,
  order: CommonJsExportOrder,
}

impl<'a> VisitJs<'a> for Scanner<'_> {
  fn visit_member_expression(&mut self, it: &MemberExpression<'a>) {
    if is_exports_object(it.object())
      && let Some(name) = it.static_property_name().and_then(oxc_str::JSStr::as_str)
    {
      let start = it.span().start;
      match next_is_equals(self.source, it.span().end) {
        Some(false) => {}
        Some(true) if is_plain_member(self.source, it) => {
          self.order.record(name, Place::Found(start));
        }
        _ => self.order.record(name, Place::Unsure(start)),
      }
    }
    walk_js::walk_member_expression(self, it);
  }

  fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
    // `module.exports = value` or `exports = value`
    let replaces_exports = match &it.left {
      AssignmentTarget::AssignmentTargetIdentifier(id) => id.name == "exports",
      left => left.as_member_expression().is_some_and(|member| {
        member.static_property_name().is_some_and(|name| name == "exports")
          && matches!(member.object(), Expression::Identifier(id) if id.name == "module")
      }),
    };
    if replaces_exports {
      self.order.replaces_exports = true;
    }
    walk_js::walk_assignment_expression(self, it);
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
    if let Expression::StaticMemberExpression(callee) = &it.callee
      && callee.property.name == "defineProperty"
      && matches!(&callee.object, Expression::Identifier(id) if id.name == "Object")
      && let [target, Argument::StringLiteral(name), ..] = it.arguments.as_slice()
      && target.as_expression().is_some_and(is_exports_object)
      && let Some(name) = name.value.as_str()
    {
      let place = if is_found_getter(self.source, it) {
        Place::Found(it.span.start)
      } else {
        Place::Unsure(it.span.start)
      };
      self.order.record(name, place);
    }
    walk_js::walk_call_expression(self, it);
  }
}

/// `exports` or `module.exports`, with any binding of `exports` and `module`.
fn is_exports_object(expr: &Expression<'_>) -> bool {
  match expr {
    Expression::Identifier(id) => id.name == "exports",
    Expression::StaticMemberExpression(member) => {
      member.property.name == "exports"
        && matches!(&member.object, Expression::Identifier(id) if id.name == "module")
    }
    _ => false,
  }
}

/// Whether the source text of `member`, a property of `exports` or `module.exports`, is one of the
/// texts that the lexer reads: `exports.name`, `exports['name']`, `module.exports.name` or
/// `module.exports['name']`, with only whitespace and comments between the tokens.
fn is_plain_member(source: &str, member: &MemberExpression<'_>) -> bool {
  let object = member.object();
  let plain_object = match object {
    Expression::Identifier(id) => text(source, id.span) == Some("exports"),
    Expression::StaticMemberExpression(object) => {
      text(source, object.object.span()) == Some("module")
        && is_gap(source, object.object.span().end, object.property.span.start, ".")
        && text(source, object.property.span) == Some("exports")
    }
    _ => false,
  };
  plain_object
    && match member {
      MemberExpression::StaticMemberExpression(member) => {
        is_gap(source, object.span().end, member.property.span.start, ".")
          && text(source, member.property.span) == Some(member.property.name.as_str())
      }
      MemberExpression::ComputedMemberExpression(member) => {
        // The lexer reads escapes in a string, but this rule does not.
        matches!(&member.expression, Expression::StringLiteral(name)
          if text(source, name.span).is_some_and(|raw| !raw.contains('\\')))
          && is_gap(source, object.span().end, member.expression.span().start, "[")
          && is_gap(source, member.expression.span().end, member.span.end, "]")
      }
      MemberExpression::PrivateFieldExpression(_) => false,
    }
}

/// Whether the lexer finds the name at `call`, a call `Object.defineProperty(target, 'name', ...)`
/// on `exports` or `module.exports`. The lexer finds these descriptors:
/// - `{ get: getter }` or `{ enumerable: true, get: getter }`, also with a comma at the end.
/// - The getter is `function () { return value; }`, also with a function name, or a method
///   `get() { return value; }`. The semicolon is optional.
/// - The value is `id`, `id.name` or `id['name']`.
///
/// The source text of the call must not have comments, escapes, `?`, characters that are not
/// ASCII, or parentheses other than the two pairs of the form. The lexer finds some of these
/// texts, but this function does not identify them.
fn is_found_getter(source: &str, call: &CallExpression<'_>) -> bool {
  let Some(call_text) = text(source, call.span) else {
    return false;
  };
  // Without comments, escapes and strings with these characters, each `(` and `)` is a token.
  if !call_text.is_ascii()
    || call_text.contains(['/', '\\', '?'])
    || call_text.bytes().filter(|byte| *byte == b'(').count() != 2
    || call_text.bytes().filter(|byte| *byte == b')').count() != 2
  {
    return false;
  }
  let [_, _, Argument::ObjectExpression(descriptor)] = call.arguments.as_slice() else {
    return false;
  };
  // The lexer does not find the name if a comma comes after the descriptor.
  if text(source, Span::new(descriptor.span.end, call.span.end)).map(str::trim_ascii) != Some(")") {
    return false;
  }
  let (enumerable, get) = match descriptor.properties.as_slice() {
    [get] => (None, get),
    [enumerable, get] => (Some(enumerable), get),
    _ => return false,
  };
  if enumerable.is_some_and(|enumerable| {
    !matches!(property_value(enumerable, "enumerable"), Some(Expression::BooleanLiteral(value)) if value.value)
  }) {
    return false;
  }
  let Some(Expression::FunctionExpression(getter)) = property_value(get, "get") else {
    return false;
  };
  let Some(body) = &getter.body else {
    return false;
  };
  !getter.r#async
    && !getter.generator
    && getter.type_parameters.is_none()
    && getter.this_param.is_none()
    && getter.return_type.is_none()
    && getter.params.items.is_empty()
    && getter.params.rest.is_none()
    && body.directives.is_empty()
    && matches!(body.statements.as_slice(), [Statement::ReturnStatement(statement)]
      if statement.argument.as_ref().is_some_and(is_plain_value))
}

/// The value of `property` if it is `key: value` or a method `key() {}` with the identifier `key`.
fn property_value<'b, 'a>(
  property: &'b ObjectPropertyKind<'a>,
  key: &str,
) -> Option<&'b Expression<'a>> {
  let ObjectPropertyKind::ObjectProperty(property) = property else {
    return None;
  };
  (property.kind == PropertyKind::Init
    && !property.shorthand
    && !property.computed
    && matches!(&property.key, PropertyKey::StaticIdentifier(id) if id.name == key))
  .then_some(&property.value)
}

/// `id`, `id.name` or `id['name']`.
fn is_plain_value(expr: &Expression<'_>) -> bool {
  match expr {
    Expression::Identifier(_) => true,
    Expression::StaticMemberExpression(member) => {
      matches!(member.object, Expression::Identifier(_))
    }
    Expression::ComputedMemberExpression(member) => {
      matches!(member.object, Expression::Identifier(_))
        && matches!(member.expression, Expression::StringLiteral(_))
    }
    _ => false,
  }
}

/// The source text of `span`, or `None` if `span` is not in `source`.
fn text(source: &str, span: Span) -> Option<&str> {
  source.get(span.start as usize..span.end as usize)
}

/// Whether the source text from `start` to `end` is `expected`, with only ASCII whitespace and
/// comments around and between its characters.
fn is_gap(source: &str, start: u32, end: u32, expected: &str) -> bool {
  let Some(gap) = source.as_bytes().get(start as usize..end as usize) else {
    return false;
  };
  let mut at = 0;
  for byte in expected.bytes() {
    match skip_trivia(gap, at) {
      Some(next) if gap.get(next) == Some(&byte) => at = next + 1,
      _ => return false,
    }
  }
  skip_trivia(gap, at) == Some(gap.len())
}

/// Whether the first character after `end` that is not whitespace and not in a comment is `=`.
/// `None` if the scan finds a character that is not ASCII first. The lexer does not always read
/// such a character as JavaScript does: for example, it reads U+00A0 as a part of a name.
fn next_is_equals(source: &str, end: u32) -> Option<bool> {
  let rest = source.as_bytes().get(end as usize..)?;
  let next = skip_trivia(rest, 0)?;
  Some(rest.get(next) == Some(&b'='))
}

/// The index of the first byte at or after `at` that is not ASCII whitespace and not in a comment.
/// `None` if a byte that is not ASCII comes first, also in a comment, or if a block comment has no
/// end.
fn skip_trivia(bytes: &[u8], mut at: usize) -> Option<usize> {
  loop {
    match bytes.get(at..)? {
      [b' ' | b'\t' | b'\n' | b'\r' | b'\x0B' | b'\x0C', ..] => at += 1,
      [b'/', b'*', rest @ ..] => {
        let len = rest.windows(2).position(|pair| pair == b"*/")?;
        if !rest[..len].is_ascii() {
          return None;
        }
        at += len + 4;
      }
      [b'/', b'/', rest @ ..] => {
        let len = rest.iter().position(|byte| matches!(byte, b'\n' | b'\r')).unwrap_or(rest.len());
        if !rest[..len].is_ascii() {
          return None;
        }
        at += len + 2;
      }
      [byte, ..] if !byte.is_ascii() => return None,
      _ => return Some(at),
    }
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    allocator::Allocator,
    parser::{ParseOptions, Parser},
    span::SourceType,
  };

  use super::CommonJsExportOrder;

  /// The names of `names` in the order of their positions, or `None` if a name has no position.
  /// The parser options are the options of the module loader, which removes parentheses.
  fn order(source: &str, names: &[&str]) -> Option<Vec<String>> {
    let allocator = Allocator::default();
    let program = Parser::new(&allocator, source, SourceType::cjs())
      .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
      .parse()
      .program;
    let order = CommonJsExportOrder::new(&program);
    let mut positions =
      names.iter().map(|name| Some((order.position(name)?, *name))).collect::<Option<Vec<_>>>()?;
    positions.sort_unstable();
    Some(positions.into_iter().map(|(_, name)| name.to_string()).collect())
  }

  /// The order of `x` and `y` for `form`, then `exports.x = 0; exports.y = 0;`.
  fn before_assignments(form: &str) -> Option<Vec<String>> {
    order(&format!("{form}\nexports.x = 0; exports.y = 0;"), &["x", "y"])
  }

  /// The order of `x` and `y` for `form`, then `exports.x = 0;`.
  fn before_x(form: &str) -> Option<Vec<String>> {
    order(&format!("{form}\nexports.x = 0;"), &["x", "y"])
  }

  /// Each form of `forms` for which `order_of` does not give `expected`, with the result.
  fn wrong_orders<S: AsRef<str>>(
    forms: &[S],
    expected: Option<[&str; 2]>,
    order_of: impl Fn(&str) -> Option<Vec<String>>,
  ) -> Vec<String> {
    forms
      .iter()
      .map(AsRef::as_ref)
      .filter_map(|form| {
        let actual = order_of(form);
        (actual != expected.map(|names| names.map(String::from).to_vec()))
          .then(|| format!("{form:?} gives {actual:?}, not {expected:?}"))
      })
      .collect()
  }

  /// A call `Object.defineProperty(exports, 'y', descriptor)`.
  fn define_y(descriptor: &str) -> String {
    format!("Object.defineProperty(exports, 'y', {descriptor});")
  }

  // Each order that a test expects is the order of the getter calls that Node v26.8.1 (CommonJS
  // lexer merve 1.2.2) gives for the same statements, with getters that record the calls. A `None`
  // result is the rule of `CommonJsExportOrder`, not a result of Node.
  #[test]
  fn first_assignment_sets_the_position() {
    assert_eq!(order("exports.x = 0; exports.y = 0;", &["x", "y"]).unwrap(), ["x", "y"]);
    assert_eq!(order("exports.y = 0; exports.x = 0;", &["x", "y"]).unwrap(), ["y", "x"]);
    assert_eq!(
      order("exports.x = 0; exports.y = 0; exports.x = 0;", &["x", "y"]).unwrap(),
      ["x", "y"]
    );
    assert_eq!(
      order("exports.y = 0; exports.x = 0; exports.y = 0;", &["x", "y"]).unwrap(),
      ["y", "x"]
    );
    assert_eq!(order("exports.b = exports.a = void 0;", &["a", "b"]).unwrap(), ["b", "a"]);
  }

  #[test]
  fn assignment_forms() {
    assert_eq!(
      order("module.exports.y = 0; module.exports.x = 0;", &["x", "y"]).unwrap(),
      ["y", "x"]
    );
    assert_eq!(order("exports['y'] = 0; exports['x'] = 0;", &["x", "y"]).unwrap(), ["y", "x"]);
    assert_eq!(
      order("module.exports.y = 0; exports.x = 0; exports.y = 0;", &["x", "y"]).unwrap(),
      ["y", "x"]
    );
    assert_eq!(
      order("function f() { exports.y = 0; } exports.x = 0; f();", &["x", "y"]).unwrap(),
      ["y", "x"]
    );
    // The lexer does not resolve scopes.
    assert_eq!(
      order(
        "(function (exports) { exports.y = 0; })({}); exports.x = 0; exports.y = 0;",
        &["x", "y"]
      )
      .unwrap(),
      ["y", "x"]
    );
  }

  #[test]
  fn equals_after_the_name() {
    // Node finds `y` at the form, because `=` comes after `exports.y`.
    let found = [
      "if (exports.y == 0) {}",
      "if (exports.y === void 0) {}",
      "if (exports['y'] == 0) {}",
      "if (module.exports.y == 0) {}",
      "if (typeof exports.y === 'undefined') {}",
      "if (!exports.y == 0) {}",
      "if (a + exports.y == 1) {}",
      "if (0 == exports.y == 1) {}",
      "[exports.y = 1] = [];",
      "({ a: exports.y = 1 } = {});",
      "(exports.y = 0);",
      "exports.y = (0);",
      "exports.y\n= 0;",
      "exports.y /* c */ = 0;",
      "exports.y // c\n= 0;",
      "exports /* c */ . y /* d */ = 0;",
      "if (exports . y == 0) {}",
    ];
    // Node finds `y` only at the later assignment.
    let later = [
      "if (exports.y != 0) {}",
      "if (exports.y >= 0) {}",
      "if (0 == exports.y) {}",
      "void exports.y;",
      "exports.y ? 0 : 0;",
      "try { exports.y.z = 0; } catch {}",
      "exports.y += '';",
      "exports.y ||= 0;",
      "exports.y ??= 0;",
      "(exports.y) = 0;",
      "if ((exports.y) == 0) {}",
    ];
    // The source text has parentheses that the parser removes, `?.`, an escape, or a character
    // that is not ASCII. Node finds `y` only at the later assignment in these forms, except in the
    // escaped form of `exports['y']`, where Node finds `y` at the form.
    let unsure = [
      "(exports).y = 0;",
      "(module.exports).y = 0;",
      "(module).exports.y = 0;",
      "if ((exports).y == 0) {}",
      "exports[('y')] = 0;",
      "if (exports?.y == 0) {}",
      "exports.\\u0079 = 0;",
      "exports['\\u0079'] = 0;",
      "exports.y\u{a0}= 0;",
      "exports.y\u{2028}= 0;",
      "exports.y // c\u{2028}= 0;",
    ];
    let wrong = [
      wrong_orders(&found, Some(["y", "x"]), before_assignments),
      wrong_orders(&later, Some(["x", "y"]), before_assignments),
      wrong_orders(&unsure, None, before_assignments),
    ]
    .concat();
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
  }

  #[test]
  fn define_property_getter_forms() {
    // Node finds `y` at the call.
    let found = [
      "{ enumerable: true, get: function () { return g.y; } }",
      "{ get: function () { return g.y; } }",
      "{ enumerable: true, get() { return g.y; } }",
      "{ get() { return g.y; } }",
      "{ enumerable: true, get: function () { return g.y } }",
      "{ enumerable: true, get: function () { return g.y; }, }",
      "{ enumerable: true, get: function named() { return g.y; } }",
      "{ enumerable: true, get: function () { return g['y']; } }",
      "{ enumerable: true, get: function () { return gy; } }",
    ]
    .map(define_y);
    let targets = [
      "Object.defineProperty(module.exports, 'y', { enumerable: true, get: function () { return g.y; } });",
      "Object.defineProperty(exports, \"y\", { enumerable: true, get: function () { return g.y; } });",
    ];
    // Node does not find `y` at these calls, so the later assignment sets the position.
    let not_found = [
      "Object.defineProperty(exports, `y`, { enumerable: true, get: function () { return g.y; } });",
      "Object.defineProperty(module?.exports, 'y', { enumerable: true, get: function () { return g.y; } });",
    ];
    let wrong = [
      wrong_orders(&found, Some(["y", "x"]), before_x),
      wrong_orders(&found, Some(["y", "x"]), before_assignments),
      wrong_orders(&targets, Some(["y", "x"]), before_x),
      wrong_orders(&not_found, Some(["x", "y"]), before_assignments),
    ]
    .concat();
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
  }

  #[test]
  fn define_property_unsure_forms() {
    // The rule does not identify these calls. Each call is an unsure place, so the position of `y`
    // is not known, also when an assignment of `y` comes later.
    let unsure = |calls: &[String]| {
      [wrong_orders(calls, None, before_x), wrong_orders(calls, None, before_assignments)].concat()
    };
    // Node does not find `y` at the call.
    let not_found = [
      "{ enumerable: true, configurable: true, get: function () { return g.y; } }",
      "{ get: function () { return g.y; }, enumerable: true }",
      "{ enumerable: !0, get: function () { return g.y; } }",
      "{ enumerable: false, get: function () { return g.y; } }",
      "{ 'enumerable': true, get: function () { return g.y; } }",
      "{ enumerable: true, 'get': function () { return g.y; } }",
      "{ enumerable: true, get: () => g.y }",
      "{ enumerable: true, get: function () { 'use strict'; return g.y; } }",
      "{ enumerable: true, get: function () { return g.y.z; } }",
      "{ enumerable: true, get: function () { return g?.y; } }",
      "{ enumerable: true, get: function (a) { return g.y; } }",
      "{ enumerable: true, get: function () { void 0; return g.y; } }",
      "{ enumerable: true, get: function () { return g.y;; } }",
      "{ enumerable: true, get: function () { return f(g).y; } }",
      "{ enumerable: true, get: async function () { return g.y; } }",
      "{ enumerable: true, get() { const v = g.y; return v; } }",
      // Parentheses that the parser removes, and an escape.
      "{ enumerable: true, get: function () { return (g.y); } }",
      "{ enumerable: true, get: function () { return (g).y; } }",
      "{ enumerable: true, get: (function () { return g.y; }) }",
      "{ enumerable: (true), get: function () { return g.y; } }",
      "({ enumerable: true, get: function () { return g.y; } })",
      "{ enumerable: true, get: function () { return g.\\u0079; } }",
    ]
    .map(define_y);
    let getter = "{ enumerable: true, get: function () { return g.y; } }";
    let parentheses = [
      format!("(Object).defineProperty(exports, 'y', {getter});"),
      format!("(Object.defineProperty)(exports, 'y', {getter});"),
      format!("Object.defineProperty((exports), 'y', {getter});"),
      format!("Object.defineProperty(exports, ('y'), {getter});"),
      format!("Object.defineProperty(exports, 'y', {getter},);"),
      format!("Object?.defineProperty(exports, 'y', {getter});"),
    ];
    // Node finds `y` at the call. The getter `this.y` reads `y` again, so its position is not
    // clear.
    let found = [
      "{ enumerable: true, /* c */ get: /* d */ function () { /* e */ return g.y; } }",
      "{ enumerable: true, get: function () { /* ( */ return g.y; } }",
      "{ enumerable: true, get: function () { return g['(']; } }",
      "{ enumerable: true, get: function () { return\ng.y; } }",
      "{ enumerable: true, get: function () { return this.y; } }",
    ]
    .map(define_y)
    .into_iter()
    .chain([
      "Object.defineProperty(exports, '\\u0079', { enumerable: true, get: function () { return g.y; } });"
        .to_string(),
    ])
    .collect::<Vec<_>>();
    let wrong = [unsure(&not_found), unsure(&parentheses), unsure(&found)].concat();
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
  }

  #[test]
  fn later_define_property_does_not_move_the_name() {
    let detected = |name: &str| {
      format!(
        "Object.defineProperty(exports, '{name}', {{ enumerable: true, get: function () {{ return g.{name}; }} }});"
      )
    };
    let opaque = |name: &str| {
      format!(
        "Object.defineProperty(exports, '{name}', {{ enumerable: true, get() {{ return ++count; }} }});"
      )
    };
    for define in [detected, opaque] {
      assert_eq!(
        order(&format!("exports.x = 0; exports.y = 0; {}", define("x")), &["x", "y"]).unwrap(),
        ["x", "y"]
      );
      assert_eq!(
        order(&format!("exports.y = 0; exports.x = 0; {}", define("y")), &["x", "y"]).unwrap(),
        ["y", "x"]
      );
    }
    // The modules `defxy` and `defyx`: the getter form, an assignment and an opaque getter.
    assert_eq!(
      order(&format!("{} exports.y = 0; {}", detected("x"), opaque("y")), &["x", "y"]).unwrap(),
      ["x", "y"]
    );
    assert_eq!(
      order(&format!("{} exports.x = 0; {}", detected("y"), opaque("x")), &["x", "y"]).unwrap(),
      ["y", "x"]
    );
  }

  #[test]
  fn unknown_order() {
    assert_eq!(order("exports.x = 0;", &["x", "y"]), None);
    assert_eq!(order("exports.x = 0; exports.y = 0; module.exports = {};", &["x", "y"]), None);
    assert_eq!(order("exports.x = 0; exports.y = 0; exports = {};", &["x", "y"]), None);
  }
}
