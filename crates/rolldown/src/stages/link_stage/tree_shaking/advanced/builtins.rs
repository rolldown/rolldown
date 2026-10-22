//! Calls of builtin methods on values that the analysis knows: array literals, primitives,
//! object literals, regular expression literals, and `Object.freeze`. Like rollup, the analysis
//! assumes that no code changes the builtins. See internal-docs/advanced-tree-shaking/design.md.

use oxc::ast::ast::{Argument, CallExpression, Expression};

use super::{
  Analyzer,
  effects::Cx,
  value::{ArrValue, Lit, Proto, Resolved, Slot, Value},
};

/// Array methods that read the elements and convert their other arguments to numbers.
const ARRAY_READ_METHODS: [&str; 5] = ["at", "includes", "indexOf", "lastIndexOf", "slice"];
/// Array methods that call their first argument for each element. They use only the truth
/// value of what the callback returns, or they store it.
const ARRAY_CALLBACK_METHODS: [&str; 9] =
  ["every", "filter", "find", "findIndex", "findLast", "findLastIndex", "forEach", "map", "some"];
/// Array methods that create a new array from the `constructor` of the array.
const ARRAY_SPECIES_METHODS: [&str; 3] = ["filter", "map", "slice"];
/// Array methods that change the array.
const ARRAY_MUTATING_METHODS: [&str; 4] = ["pop", "push", "shift", "unshift"];
/// String methods that cannot throw and run no user code when the arguments are primitives.
/// `normalize`, `padEnd`, `padStart`, and `repeat` can throw a `RangeError`, and `match`,
/// `matchAll`, and `search` compile their argument as a regular expression.
const STRING_METHODS: [&str; 25] = [
  "at",
  "charAt",
  "charCodeAt",
  "codePointAt",
  "concat",
  "endsWith",
  "includes",
  "indexOf",
  "isWellFormed",
  "lastIndexOf",
  "replace",
  "replaceAll",
  "slice",
  "split",
  "startsWith",
  "substr",
  "substring",
  "toLowerCase",
  "toString",
  "toUpperCase",
  "toWellFormed",
  "trim",
  "trimEnd",
  "trimStart",
  "valueOf",
];
/// Methods of `Object.prototype` that convert their argument to a property key at most.
const OBJECT_METHODS: [&str; 4] =
  ["hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "valueOf"];

impl<'a> Analyzer<'_, 'a> {
  /// The effects of a call of a builtin method of a known value, apart from the effects of the
  /// callee expression and the arguments themselves. `None` when the call is not such a call.
  pub fn builtin_call_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> Option<bool> {
    if self.is_global_symbol_call(call, cx.site.module) {
      // The description converts to a string.
      return Some(!self.primitive_args(call, cx));
    }
    let member = call.callee.without_parentheses().as_member_expression()?;
    let method = self.member_key(member, cx.site)?;
    let method = method.as_str();
    let object = member.object().without_parentheses();
    if let Expression::RegExpLiteral(_) = object {
      // Each evaluation of the literal creates a new object.
      return (matches!(method, "exec" | "test" | "toString") && self.primitive_args(call, cx))
        .then_some(false);
    }
    if let Expression::Identifier(ident) = object
      && ident.name == "Object"
      && matches!(self.resolve(cx.site.module, ident), Resolved::Global)
    {
      return match method {
        "freeze" => self.freeze_effects(call, cx),
        _ => None,
      };
    }
    match self.value(object, cx.site) {
      Value::Lit(Lit::Str(_)) => {
        (STRING_METHODS.contains(&method) && self.primitive_args(call, cx)).then_some(false)
      }
      Value::Lit(Lit::Number(_)) => self.number_method_ok(call, method, cx).then_some(false),
      Value::Lit(Lit::Bool(_)) => (matches!(method, "toString" | "valueOf")).then_some(false),
      Value::Array(arr) if !self.is_array_escaped(&arr) => {
        self.array_call_effects(call, arr, method, cx)
      }
      Value::Object(obj) if !self.is_object_escaped(&obj) => {
        let inherited = OBJECT_METHODS.contains(&method)
          && !self.is_key_deopted(&obj, method)
          && matches!(self.own_slot(&obj, method), Slot::Missing)
          && matches!(self.proto_of(&obj), Proto::Default);
        (inherited && self.primitive_args(call, cx)).then_some(false)
      }
      _ => None,
    }
  }

  fn array_call_effects(
    &mut self,
    call: &'a CallExpression<'a>,
    arr: ArrValue<'a>,
    method: &str,
    cx: Cx<'a>,
  ) -> Option<bool> {
    // A write can set `constructor`, which decides the class of the new array.
    if ARRAY_SPECIES_METHODS.contains(&method)
      && self.state.entity(arr.key()).is_some_and(|entity| entity.all_keys_deopted)
    {
      return None;
    }
    if ARRAY_MUTATING_METHODS.contains(&method) {
      if call.arguments.iter().any(|arg| matches!(arg, Argument::SpreadElement(_))) {
        return None;
      }
      // The call changes the length and the elements.
      let member = call.callee.without_parentheses().as_member_expression()?;
      self.deopt_array_change(member.object(), cx);
      return Some(self.array_write_effects(&arr));
    }
    if method == "join" {
      let elements_primitive = arr.node.elements.iter().all(|element| match element {
        oxc::ast::ast::ArrayExpressionElement::SpreadElement(_) => false,
        oxc::ast::ast::ArrayExpressionElement::Elision(_) => true,
        element => matches!(self.value(element.to_expression(), arr.site), Value::Lit(_)),
      });
      return (elements_primitive
        && !self.state.entity(arr.key()).is_some_and(|entity| entity.all_keys_deopted)
        && self.primitive_args(call, cx))
      .then_some(false);
    }
    if ARRAY_READ_METHODS.contains(&method) {
      // `includes`, `indexOf`, and `lastIndexOf` compare their first argument without a
      // conversion.
      let skip = usize::from(matches!(method, "includes" | "indexOf" | "lastIndexOf"));
      return self.primitive_args_from(call, skip, cx).then_some(false);
    }
    if ARRAY_CALLBACK_METHODS.contains(&method) {
      let callback = match call.arguments.first() {
        Some(arg) if !matches!(arg, Argument::SpreadElement(_)) => arg.to_expression(),
        _ => return None,
      };
      let Value::Function(func) = self.value(callback, cx.site) else {
        return None;
      };
      // The builtin calls the callback with arguments that the analysis does not record.
      self.state.escape_entity(func.key());
      return Some(self.function_call_effects(func));
    }
    None
  }

  /// `Object.freeze(o)` makes each later write to `o` throw. A frozen object makes such writes
  /// effects, so the call has an effect only when included code can write the object. A literal
  /// in the argument position has no other reference.
  fn freeze_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> Option<bool> {
    let [arg] = call.arguments.as_slice() else {
      return None;
    };
    let arg = arg.as_expression()?;
    match self.value(arg, cx.site) {
      Value::Lit(_) => Some(false),
      Value::Object(obj) if !self.is_object_escaped(&obj) => {
        let included = match obj.holder {
          Some(holder) => self.state.is_included(holder),
          None if matches!(arg.without_parentheses(), Expression::ObjectExpression(_)) => false,
          None => return None,
        };
        self.state.freeze_entity(obj.key());
        Some(included)
      }
      _ => None,
    }
  }

  fn number_method_ok(&mut self, call: &'a CallExpression<'a>, method: &str, cx: Cx<'a>) -> bool {
    // The ranges that do not throw a `RangeError`.
    let (min, max) = match method {
      "valueOf" => return true,
      "toString" => (2.0, 36.0),
      "toFixed" | "toExponential" => (0.0, 100.0),
      "toPrecision" => (1.0, 100.0),
      _ => return false,
    };
    match call.arguments.as_slice() {
      [] => true,
      [arg] => match arg.as_expression().map(|arg| self.value(arg, cx.site)) {
        Some(Value::Lit(Lit::Undefined)) => true,
        Some(Value::Lit(Lit::Number(value))) => (min..=max).contains(&value.trunc()),
        _ => false,
      },
      _ => false,
    }
  }

  /// Whether every argument is a primitive value, so its conversion runs no user code.
  fn primitive_args(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> bool {
    self.primitive_args_from(call, 0, cx)
  }

  fn primitive_args_from(&mut self, call: &'a CallExpression<'a>, skip: usize, cx: Cx<'a>) -> bool {
    call.arguments.iter().enumerate().all(|(index, arg)| match arg {
      Argument::SpreadElement(_) => false,
      _ if index < skip => true,
      arg => matches!(self.value(arg.to_expression(), cx.site), Value::Lit(_)),
    })
  }
}
