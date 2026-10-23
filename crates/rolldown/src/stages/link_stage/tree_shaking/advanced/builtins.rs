//! Calls of builtin methods on values that the analysis knows: array literals, primitives,
//! object literals, regular expression literals, some methods of `Object`, and `new` of typed
//! arrays. Like rollup, the analysis assumes that no code changes the builtins. See
//! internal-docs/advanced-tree-shaking/design.md.

use oxc::ast::ast::{Argument, ArrayExpressionElement, CallExpression, Expression, NewExpression};

use super::{
  Analyzer,
  effects::Cx,
  facts::CallArgs,
  value::{ArrValue, FnValue, Lit, Proto, RegExpValue, Resolved, Slot, Value},
};

/// Array methods that read the elements and convert their other arguments to numbers.
const ARRAY_READ_METHODS: [&str; 5] = ["at", "includes", "indexOf", "lastIndexOf", "slice"];
/// Array methods that call their first argument for each element. They use only the truth
/// value of what the callback returns, or they store it.
const ARRAY_CALLBACK_METHODS: [&str; 9] =
  ["every", "filter", "find", "findIndex", "findLast", "findLastIndex", "forEach", "map", "some"];
/// Array methods that create a new array from the `constructor` of the array.
const ARRAY_SPECIES_METHODS: [&str; 7] =
  ["concat", "filter", "flat", "flatMap", "map", "slice", "splice"];
/// Array methods that change the array and store their arguments.
const ARRAY_MUTATING_METHODS: [&str; 5] = ["pop", "push", "reverse", "shift", "unshift"];
/// Array methods that change the array, but only move or remove elements.
const ARRAY_REORDER_METHODS: [&str; 5] = ["copyWithin", "pop", "reverse", "shift", "sort"];
/// String methods that cannot throw and run no user code when the arguments are primitives.
/// `string_method_ok` checks the other methods, which can throw a `RangeError` or compile their
/// argument as a regular expression.
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
/// Typed array constructors whose elements convert with `ToNumber`.
const TYPED_ARRAYS: [&str; 9] = [
  "Float32Array",
  "Float64Array",
  "Int16Array",
  "Int32Array",
  "Int8Array",
  "Uint16Array",
  "Uint32Array",
  "Uint8Array",
  "Uint8ClampedArray",
];
/// Typed array constructors whose elements convert with `ToBigInt`, which throws for a number.
const BIGINT_TYPED_ARRAYS: [&str; 2] = ["BigInt64Array", "BigUint64Array"];
/// The longest string that every engine can create. A longer result throws a `RangeError`.
const MAX_STRING_LENGTH: f64 = 268_435_440.0;
/// The longest typed array that `new` of a typed array creates without a check of memory.
const MAX_TYPED_ARRAY_LENGTH: u32 = 65_536;

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
        "entries" | "values" => self.own_values_effects(call, cx),
        _ => None,
      };
    }
    match self.value(object, cx.site) {
      Value::Lit(Lit::Str(string)) => {
        let ok = if STRING_METHODS.contains(&method) {
          self.primitive_args(call, cx)
        } else {
          self.string_method_ok(call, &string, method, cx)
        };
        ok.then_some(false)
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
      Value::RegExp(re) if !self.is_regexp_escaped(&re) => match method {
        "toString" => Some(false),
        "exec" | "test" => self
          .primitive_args(call, cx)
          .then(|| re.writes_last_index() && self.regexp_write_effects(&re)),
        _ => None,
      },
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
    let object = call.callee.without_parentheses().as_member_expression()?.object();
    let mutating = ARRAY_MUTATING_METHODS.contains(&method)
      || matches!(method, "copyWithin" | "fill" | "sort" | "splice");
    if mutating {
      // The call changes the length and the elements, unless it throws first. `splice` stores
      // the arguments after the first two.
      if ARRAY_REORDER_METHODS.contains(&method)
        || (method == "splice" && call.arguments.len() <= 2)
      {
        self.deopt_array_reorder(object, cx);
      } else {
        self.deopt_array_change(object, cx);
      }
    }
    // A write can replace the method, or set `constructor`, which decides the class of the new
    // array.
    if self.is_array_key_written(&arr, method)
      || (ARRAY_SPECIES_METHODS.contains(&method) && self.is_array_key_written(&arr, "constructor"))
      || call.arguments.iter().any(|arg| matches!(arg, Argument::SpreadElement(_)))
    {
      return None;
    }
    let ok = match method {
      m if ARRAY_MUTATING_METHODS.contains(&m) => true,
      // These methods convert the arguments at the given positions to numbers.
      "copyWithin" => self.primitive_args_in(call, 0, 3, cx),
      "fill" => self.primitive_args_in(call, 1, 3, cx),
      "splice" => self.primitive_args_in(call, 0, 2, cx),
      "sort" => match self.callback(call, cx) {
        // The default comparison converts the elements to strings.
        Ok(None) => self.element_join(&arr).is_primitive(),
        // The sort converts what the comparator returns to a number.
        Ok(Some(func)) => {
          self.record_args(&func, call.node_id(), CallArgs::Compare { arr }, cx.site);
          if self.function_call_effects(func) {
            return Some(true);
          }
          self.return_value(func).is_primitive()
        }
        Err(()) => false,
      },
      _ => return self.array_read_call_effects(call, arr, method, cx),
    };
    ok.then(|| self.array_write_effects(&arr))
  }

  /// The effects of a call of an array method that does not change the array.
  fn array_read_call_effects(
    &mut self,
    call: &'a CallExpression<'a>,
    arr: ArrValue<'a>,
    method: &str,
    cx: Cx<'a>,
  ) -> Option<bool> {
    match method {
      "join" => {
        let ok = self.is_array_values_known(&arr)
          && arr.node.elements.iter().all(|element| match element {
            ArrayExpressionElement::SpreadElement(_) => false,
            ArrayExpressionElement::Elision(_) => true,
            element => self.value(element.to_expression(), arr.site).is_primitive(),
          })
          && self.primitive_args(call, cx);
        ok.then_some(false)
      }
      // A primitive is appended, and an array literal gives its elements. Another object can
      // define `Symbol.isConcatSpreadable`.
      "concat" => call
        .arguments
        .iter()
        .all(|arg| match self.value(arg.to_expression(), cx.site) {
          Value::Array(arg) => !self.is_array_escaped(&arg),
          value => value.is_primitive(),
        })
        .then_some(false),
      // Only a depth of at most one: then the flattening reads the elements of the elements,
      // and copies what it reads.
      "flat" => {
        let depth_ok =
          match call.arguments.first().map(|arg| self.value(arg.to_expression(), cx.site)) {
            None | Some(Value::Lit(Lit::Undefined)) => true,
            Some(Value::Lit(Lit::Number(depth))) => depth.is_nan() || depth.trunc() <= 1.0,
            _ => false,
          };
        (depth_ok && self.elements_flattenable(&arr)).then_some(false)
      }
      "reduce" | "reduceRight" => {
        let func = self.callback(call, cx).ok()??;
        let initial = call.arguments.get(1).map(Argument::to_expression);
        // Without a start value, an array with no element throws.
        if initial.is_none() && !self.has_element(&arr) {
          return None;
        }
        self.record_args(
          &func,
          call.node_id(),
          CallArgs::Reduce { arr, initial, callback: func },
          cx.site,
        );
        Some(self.function_call_effects(func))
      }
      "flatMap" => {
        let func = self.callback(call, cx).ok()??;
        self.record_args(&func, call.node_id(), CallArgs::Each { arr }, cx.site);
        // The flattening reads the elements of a returned array.
        if self.function_call_effects(func) {
          return Some(true);
        }
        let returned = self.return_value(func);
        Some(!self.is_flattenable(&returned))
      }
      m if ARRAY_CALLBACK_METHODS.contains(&m) => {
        let func = self.callback(call, cx).ok()??;
        self.record_args(&func, call.node_id(), CallArgs::Each { arr }, cx.site);
        Some(self.function_call_effects(func))
      }
      m if ARRAY_READ_METHODS.contains(&m) => {
        // `includes`, `indexOf`, and `lastIndexOf` compare their first argument without a
        // conversion.
        let skip = usize::from(matches!(m, "includes" | "indexOf" | "lastIndexOf"));
        self.primitive_args_from(call, skip, cx).then_some(false)
      }
      _ => None,
    }
  }

  /// Whether a visited write can have set the property `key` of the arrays of the literal. A
  /// write of an index or of `length` changes only the elements, and a write with an unknown key
  /// makes the arrays escape.
  fn is_array_key_written(&self, arr: &ArrValue<'a>, key: &str) -> bool {
    self.state.entity(arr.key()).is_some_and(|entity| entity.deopted_keys.contains(key))
  }

  /// The function in the first argument. `Ok(None)` when the argument is absent or
  /// `undefined`, and `Err` when it is another value.
  fn callback(
    &mut self,
    call: &'a CallExpression<'a>,
    cx: Cx<'a>,
  ) -> Result<Option<FnValue<'a>>, ()> {
    match call.arguments.first().map(|arg| self.value(arg.to_expression(), cx.site)) {
      None | Some(Value::Lit(Lit::Undefined)) => Ok(None),
      Some(Value::Function(func)) => Ok(Some(func)),
      _ => Err(()),
    }
  }

  /// Whether the arrays of the literal have an element that is not a hole.
  fn has_element(&self, arr: &ArrValue<'a>) -> bool {
    self.is_array_shape_known(arr)
      && arr
        .node
        .elements
        .iter()
        .any(|element| !matches!(element, ArrayExpressionElement::Elision(_)))
  }

  /// Whether flattening each element of the arrays of the literal one level runs no user code.
  fn elements_flattenable(&mut self, arr: &ArrValue<'a>) -> bool {
    self.is_array_values_known(arr)
      && arr.node.elements.iter().all(|element| match element {
        ArrayExpressionElement::SpreadElement(_) => false,
        ArrayExpressionElement::Elision(_) => true,
        element => {
          let value = self.init_value(element.to_expression(), arr.site, arr.holder);
          self.is_flattenable(&value)
        }
      })
  }

  /// Whether flattening `value` one level runs no user code. An array literal gives its
  /// elements, and another value that the analysis knows is not an array, so it stays.
  fn is_flattenable(&self, value: &Value<'a>) -> bool {
    match value {
      Value::Unknown => false,
      Value::Array(arr) => !self.is_array_escaped(arr),
      _ => true,
    }
  }

  /// `Object.freeze(o)` makes each later write to `o` throw. A frozen object makes such writes
  /// effects, so the call has an effect only when included code can write the object. A literal
  /// in the argument position has no other reference.
  fn freeze_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> Option<bool> {
    let [arg] = call.arguments.as_slice() else {
      return None;
    };
    let arg = arg.as_expression()?;
    if matches!(
      arg.without_parentheses(),
      Expression::ArrayExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::RegExpLiteral(_)
    ) {
      return Some(false);
    }
    match self.value(arg, cx.site) {
      Value::Lit(_) | Value::Prim => Some(false),
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

  /// `Object.values(o)` and `Object.entries(o)` read each own enumerable property of `o`. Only
  /// a getter runs code, and `null` or `undefined` throws.
  fn own_values_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> Option<bool> {
    let arg = call.arguments.first()?.as_expression()?;
    match self.value(arg, cx.site) {
      Value::Lit(Lit::Undefined | Lit::Null) => Some(true),
      Value::Lit(_) => Some(false),
      Value::Object(obj) if !self.is_object_escaped(&obj) => {
        Some(self.object_getter_at(&obj, None))
      }
      Value::Array(arr) if !self.is_array_escaped(&arr) => Some(false),
      _ => None,
    }
  }

  /// The effects of `new` of a builtin constructor. `None` when the analysis does not model it.
  pub fn builtin_new_effects(&mut self, new: &'a NewExpression<'a>, cx: Cx<'a>) -> Option<bool> {
    let Expression::Identifier(ident) = new.callee.without_parentheses() else {
      return None;
    };
    let name = ident.name.as_str();
    let bigint = BIGINT_TYPED_ARRAYS.contains(&name);
    if !(bigint || TYPED_ARRAYS.contains(&name))
      || !matches!(self.resolve(cx.site.module, ident), Resolved::Global)
    {
      return None;
    }
    // A primitive is a length, and a length out of range throws a `RangeError`. An array literal
    // gives elements that convert. Other arguments only count for a buffer. The analysis decides
    // each `new` of a typed array, because oxc treats a length out of range as pure.
    let ok = match new.arguments.first() {
      None => true,
      Some(arg) => match arg.as_expression().map(|arg| self.value(arg, cx.site)) {
        Some(Value::Lit(Lit::Undefined | Lit::Null | Lit::Bool(_))) => true,
        Some(Value::Lit(Lit::Number(length))) => {
          length.fract() == 0.0 && (0.0..=f64::from(MAX_TYPED_ARRAY_LENGTH)).contains(&length)
        }
        Some(Value::Array(arr)) if !bigint => {
          !self.is_array_escaped(&arr)
            && self.is_array_values_known(&arr)
            && arr.node.elements.len() <= MAX_TYPED_ARRAY_LENGTH as usize
            && arr.node.elements.iter().all(|element| match element {
              ArrayExpressionElement::SpreadElement(_) => false,
              ArrayExpressionElement::Elision(_) => true,
              element => {
                self.init_value(element.to_expression(), arr.site, arr.holder).is_primitive()
              }
            })
        }
        _ => false,
      },
    };
    Some(!ok)
  }

  /// Whether a call of a string method that `STRING_METHODS` does not list cannot throw and runs
  /// no user code.
  fn string_method_ok(
    &mut self,
    call: &'a CallExpression<'a>,
    string: &str,
    method: &str,
    cx: Cx<'a>,
  ) -> bool {
    if call.arguments.iter().any(|arg| matches!(arg, Argument::SpreadElement(_))) {
      return false;
    }
    let first = call.arguments.first().map(|arg| self.value(arg.to_expression(), cx.site));
    match method {
      "repeat" => {
        let Some(count) = first.map_or(Some(0.0), |count| integer_arg(&count)) else {
          return false;
        };
        #[expect(clippy::cast_precision_loss)]
        let length = string.encode_utf16().count() as f64;
        count >= 0.0 && count.is_finite() && length * count <= MAX_STRING_LENGTH
      }
      "padStart" | "padEnd" => {
        let Some(max_length) = first.map_or(Some(0.0), |length| integer_arg(&length)) else {
          return false;
        };
        max_length <= MAX_STRING_LENGTH && self.primitive_args_in(call, 1, 2, cx)
      }
      "normalize" => match first {
        None | Some(Value::Lit(Lit::Undefined)) => true,
        Some(Value::Lit(Lit::Str(form))) => {
          matches!(form.as_str(), "NFC" | "NFD" | "NFKC" | "NFKD")
        }
        _ => false,
      },
      // A primitive argument compiles as a pattern. A regular expression runs its builtin
      // methods, and a global or sticky one writes `lastIndex` in `match`.
      "match" | "search" => match first {
        Some(Value::Lit(Lit::Str(pattern))) => is_plain_pattern(&pattern),
        None | Some(Value::Lit(_)) => true,
        Some(Value::RegExp(re)) => {
          !self.is_regexp_escaped(&re)
            && (method != "match" || !re.writes_last_index() || !self.regexp_write_effects(&re))
        }
        _ => false,
      },
      // A regular expression without the `g` flag throws. The method copies the expression, so
      // it does not write `lastIndex`.
      "matchAll" => match first {
        Some(Value::Lit(Lit::Str(pattern))) => is_plain_pattern(&pattern),
        None | Some(Value::Lit(_)) => true,
        Some(Value::RegExp(re)) => !self.is_regexp_escaped(&re) && re.is_global(),
        _ => false,
      },
      // Locales and options can throw a `RangeError`.
      "localeCompare" => call.arguments.len() <= 1 && self.primitive_args(call, cx),
      "toLocaleLowerCase" | "toLocaleUpperCase" => match first {
        None | Some(Value::Lit(Lit::Undefined)) => true,
        Some(Value::Lit(Lit::Str(tag))) => is_simple_language_tag(&tag),
        _ => false,
      },
      _ => false,
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
    // The methods ignore the other arguments.
    match call.arguments.first() {
      None => true,
      Some(arg) => match arg.as_expression().map(|arg| self.value(arg, cx.site)) {
        Some(Value::Lit(Lit::Undefined)) => true,
        Some(Value::Lit(Lit::Number(value))) => (min..=max).contains(&value.trunc()),
        _ => false,
      },
    }
  }

  /// Whether a change of the regular expression object, such as a write of `lastIndex`, is
  /// visible: included code can read the object.
  pub fn regexp_write_effects(&self, re: &RegExpValue<'a>) -> bool {
    self.is_regexp_escaped(re)
      || match re.holder {
        Some(holder) => self.state.is_included(holder),
        None => self
          .state
          .entity(re.key())
          .is_some_and(|entity| entity.demand.whole || !entity.demand.children.is_empty()),
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
      arg => self.value(arg.to_expression(), cx.site).is_primitive(),
    })
  }

  /// Whether the arguments at the positions from `start` to `end` (not included) are primitive
  /// values. The method ignores or stores the other arguments.
  fn primitive_args_in(
    &mut self,
    call: &'a CallExpression<'a>,
    start: usize,
    end: usize,
    cx: Cx<'a>,
  ) -> bool {
    call.arguments.iter().enumerate().all(|(index, arg)| match arg {
      Argument::SpreadElement(_) => false,
      _ if index < start || index >= end => true,
      arg => self.value(arg.to_expression(), cx.site).is_primitive(),
    })
  }
}

/// `ToIntegerOrInfinity` of a literal argument. `None` for a value that the analysis does not
/// convert.
fn integer_arg(value: &Value) -> Option<f64> {
  let Value::Lit(lit) = value else {
    return None;
  };
  let number = lit.to_number()?;
  Some(if number.is_nan() { 0.0 } else { number.trunc() })
}

/// Whether a string compiles as a regular expression without a syntax error: it has no
/// character that has a special meaning in a pattern, apart from ones that are valid alone.
fn is_plain_pattern(pattern: &str) -> bool {
  !pattern.contains(['\\', '(', ')', '[', ']', '{', '}', '*', '+', '?'])
}

/// Whether `Intl` accepts a locale tag: a language, an optional script, and an optional region.
fn is_simple_language_tag(tag: &str) -> bool {
  let mut parts = tag.split('-');
  let language = parts.next().unwrap_or_default();
  if !(2..=3).contains(&language.len()) || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
  {
    return false;
  }
  let mut rest = parts.peekable();
  if let Some(script) = rest.peek()
    && script.len() == 4
    && script.bytes().all(|byte| byte.is_ascii_alphabetic())
  {
    rest.next();
  }
  if let Some(region) = rest.next() {
    let valid = (region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_alphabetic()))
      || (region.len() == 3 && region.bytes().all(|byte| byte.is_ascii_digit()));
    if !valid {
      return false;
    }
  }
  rest.next().is_none()
}
