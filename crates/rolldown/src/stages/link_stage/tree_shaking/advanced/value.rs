use oxc::{
  ast::ast::{
    Argument, ArrayExpression, ArrayExpressionElement, ArrowFunctionExpression, BinaryExpression,
    BinaryOperator, BindingPattern, BindingProperty, CallExpression, ChainElement, Class,
    ConditionalExpression, Expression, FormalParameters, Function, IdentifierReference,
    LogicalExpression, LogicalOperator, MemberExpression, ObjectExpression, ObjectProperty,
    ObjectPropertyKind, PropertyKey, PropertyKind, Statement, UnaryExpression, UnaryOperator,
  },
  semantic::{NodeId, ScopeId, SymbolId},
};
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, StmtInfoIdx, SymbolRef};

use super::{
  Analyzer, collect,
  env::{DeclKind, Init, Site},
  facts::{CallRecord, LocalKey, ParamInfo},
};

/// Keys of a function whose reads can lead to calls that the analysis does not see.
const CALL_KEYS: [&str; 5] = ["apply", "arguments", "bind", "call", "caller"];

/// A deeper evaluation gives up and returns `Value::Unknown`.
pub const MAX_DEPTH: u32 = 64;

#[derive(Debug, Clone)]
pub enum Lit {
  Undefined,
  Null,
  Bool(bool),
  Number(f64),
  Str(CompactStr),
}

impl Lit {
  pub fn truthy(&self) -> bool {
    match self {
      Lit::Undefined | Lit::Null => false,
      Lit::Bool(value) => *value,
      Lit::Number(value) => *value != 0.0 && !value.is_nan(),
      Lit::Str(value) => !value.is_empty(),
    }
  }

  /// The same value, in the sense of `Object.is`.
  fn same(&self, other: &Lit) -> bool {
    match (self, other) {
      (Lit::Number(a), Lit::Number(b)) => a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
      _ => self.strict_eq(other),
    }
  }

  fn strict_eq(&self, other: &Lit) -> bool {
    match (self, other) {
      (Lit::Undefined, Lit::Undefined) | (Lit::Null, Lit::Null) => true,
      (Lit::Bool(a), Lit::Bool(b)) => a == b,
      (Lit::Number(a), Lit::Number(b)) => a == b,
      (Lit::Str(a), Lit::Str(b)) => a == b,
      _ => false,
    }
  }

  fn loose_eq(&self, other: &Lit) -> Option<bool> {
    match (self, other) {
      (Lit::Undefined | Lit::Null, Lit::Undefined | Lit::Null) => Some(true),
      (Lit::Undefined | Lit::Null, _) | (_, Lit::Undefined | Lit::Null) => Some(false),
      (Lit::Bool(_), Lit::Bool(_))
      | (Lit::Number(_), Lit::Number(_))
      | (Lit::Str(_), Lit::Str(_)) => Some(self.strict_eq(other)),
      _ => None,
    }
  }

  fn type_of(&self) -> &'static str {
    match self {
      Lit::Undefined => "undefined",
      Lit::Null => "object",
      Lit::Bool(_) => "boolean",
      Lit::Number(_) => "number",
      Lit::Str(_) => "string",
    }
  }

  fn to_number(&self) -> Option<f64> {
    match self {
      Lit::Undefined => Some(f64::NAN),
      Lit::Null => Some(0.0),
      Lit::Bool(value) => Some(f64::from(u8::from(*value))),
      Lit::Number(value) => Some(*value),
      Lit::Str(_) => None,
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub enum FnNode<'a> {
  Function(&'a Function<'a>),
  Arrow(&'a ArrowFunctionExpression<'a>),
}

impl<'a> FnNode<'a> {
  pub fn node_id(self) -> NodeId {
    match self {
      FnNode::Function(func) => func.node_id(),
      FnNode::Arrow(arrow) => arrow.node_id(),
    }
  }

  pub fn params(self) -> &'a FormalParameters<'a> {
    match self {
      FnNode::Function(func) => &func.params,
      FnNode::Arrow(arrow) => &arrow.params,
    }
  }

  pub fn scope_id(self) -> Option<ScopeId> {
    match self {
      FnNode::Function(func) => func.scope_id.get(),
      FnNode::Arrow(arrow) => arrow.scope_id.get(),
    }
  }

  pub fn is_async(self) -> bool {
    match self {
      FnNode::Function(func) => func.r#async,
      FnNode::Arrow(arrow) => arrow.r#async,
    }
  }

  pub fn is_generator(self) -> bool {
    matches!(self, FnNode::Function(func) if func.generator)
  }

  /// The body of a concise arrow function.
  pub fn expression_body(self) -> Option<&'a Expression<'a>> {
    match self {
      FnNode::Function(_) => None,
      FnNode::Arrow(arrow) => arrow.get_expression(),
    }
  }

  pub fn body_statements(self) -> &'a [Statement<'a>] {
    match self {
      FnNode::Function(func) => func.body.as_ref().map_or(&[], |body| body.statements.as_slice()),
      FnNode::Arrow(arrow) => {
        arrow.get_function_body().map_or(&[], |body| body.statements.as_slice())
      }
    }
  }
}

/// An object literal. All objects that one literal creates share one entity, so the facts about
/// the entity hold for each of them.
#[derive(Debug, Clone, Copy)]
pub struct ObjValue<'a> {
  /// Where the literal runs, which is also where its property values run.
  pub site: Site,
  pub node: &'a ObjectExpression<'a>,
  /// The canonical top-level binding whose initializer contains the literal, directly or below a
  /// property path. Such a literal runs one time. `None` for a literal in another position, which
  /// can run more than one time, so its objects have no single identity.
  pub holder: Option<SymbolRef>,
}

impl ObjValue<'_> {
  pub fn key(&self) -> (ModuleIdx, NodeId) {
    (self.site.module, self.node.node_id())
  }
}

/// An array literal. Like an object literal, all arrays that one literal creates share one
/// entity. A write to an array can change its length, so any write makes every element unknown.
#[derive(Debug, Clone, Copy)]
pub struct ArrValue<'a> {
  pub site: Site,
  pub node: &'a ArrayExpression<'a>,
  pub holder: Option<SymbolRef>,
}

impl ArrValue<'_> {
  pub fn key(&self) -> (ModuleIdx, NodeId) {
    (self.site.module, self.node.node_id())
  }
}

/// The index that a property key names, if the key is an array index.
pub fn array_index(key: &str) -> Option<usize> {
  let index = key.parse::<u32>().ok().filter(|index| *index != u32::MAX)?;
  (index.to_string() == key).then_some(index as usize)
}

#[derive(Debug, Clone, Copy)]
pub struct FnValue<'a> {
  pub module: ModuleIdx,
  /// The top-level statement that contains the function.
  pub stmt: StmtInfoIdx,
  pub node: FnNode<'a>,
  /// The canonical binding that holds the function. `None` when the function comes from an
  /// expression that can run more than one time.
  pub holder: Option<SymbolRef>,
}

impl FnValue<'_> {
  pub fn key(&self) -> (ModuleIdx, NodeId) {
    (self.module, self.node.node_id())
  }

  pub fn body_site(&self) -> Site {
    Site { module: self.module, stmt: self.stmt, in_function: true }
  }
}

/// A class. Like an object literal, all classes that one class node creates share one entity.
/// See internal-docs/advanced-tree-shaking/implementation.md.
#[derive(Debug, Clone, Copy)]
pub struct ClassValue<'a> {
  /// Where the class definition runs, which is also where its static fields run.
  pub site: Site,
  pub node: &'a Class<'a>,
  /// The canonical top-level binding that holds the class. `None` for a class from an
  /// expression that can run more than one time.
  pub holder: Option<SymbolRef>,
}

impl ClassValue<'_> {
  pub fn key(&self) -> (ModuleIdx, NodeId) {
    (self.site.module, self.node.node_id())
  }
}

#[derive(Debug, Clone)]
pub enum Value<'a> {
  Unknown,
  Lit(Lit),
  Object(ObjValue<'a>),
  Array(ArrValue<'a>),
  Function(FnValue<'a>),
  Class(ClassValue<'a>),
  /// The namespace object of an analyzed ES module.
  Namespace(ModuleIdx),
  /// A symbol from a call of the global `Symbol`. Each evaluation of the call gives a new
  /// symbol, so the analysis knows only that the value is a symbol.
  Symbol,
}

/// Whether converting `value` to a property key runs no user code.
pub fn is_primitive_key(value: &Value) -> bool {
  matches!(value, Value::Lit(_) | Value::Symbol)
}

impl Value<'_> {
  pub fn truthy(&self) -> Option<bool> {
    match self {
      Value::Unknown => None,
      Value::Lit(lit) => Some(lit.truthy()),
      Value::Object(_)
      | Value::Array(_)
      | Value::Function(_)
      | Value::Class(_)
      | Value::Namespace(_)
      | Value::Symbol => Some(true),
    }
  }

  pub fn nullish(&self) -> Option<bool> {
    match self {
      Value::Unknown => None,
      Value::Lit(lit) => Some(matches!(lit, Lit::Undefined | Lit::Null)),
      Value::Object(_)
      | Value::Array(_)
      | Value::Function(_)
      | Value::Class(_)
      | Value::Namespace(_)
      | Value::Symbol => Some(false),
    }
  }

  fn same(&self, other: &Value) -> bool {
    match (self, other) {
      (Value::Lit(a), Value::Lit(b)) => a.same(b),
      (Value::Object(a), Value::Object(b)) => {
        a.key() == b.key() && a.holder.is_some() && a.holder == b.holder
      }
      (Value::Array(a), Value::Array(b)) => {
        a.key() == b.key() && a.holder.is_some() && a.holder == b.holder
      }
      (Value::Function(a), Value::Function(b)) => {
        a.key() == b.key() && a.holder.is_some() && a.holder == b.holder
      }
      (Value::Class(a), Value::Class(b)) => {
        a.key() == b.key() && a.holder.is_some() && a.holder == b.holder
      }
      (Value::Namespace(a), Value::Namespace(b)) => a == b,
      _ => false,
    }
  }
}

fn strict_equals(left: &Value, right: &Value) -> Option<bool> {
  match (left, right) {
    (Value::Lit(a), Value::Lit(b)) => Some(a.strict_eq(b)),
    (
      Value::Lit(_),
      Value::Object(_)
      | Value::Array(_)
      | Value::Function(_)
      | Value::Class(_)
      | Value::Namespace(_),
    )
    | (
      Value::Object(_)
      | Value::Array(_)
      | Value::Function(_)
      | Value::Class(_)
      | Value::Namespace(_),
      Value::Lit(_),
    ) => Some(false),
    (Value::Object(a), Value::Object(b)) if a.holder.is_some() && b.holder.is_some() => {
      Some(a.key() == b.key())
    }
    (Value::Array(a), Value::Array(b)) if a.holder.is_some() && b.holder.is_some() => {
      Some(a.key() == b.key())
    }
    (Value::Namespace(a), Value::Namespace(b)) => Some(a == b),
    _ => None,
  }
}

/// Merge `value` into the join `acc` of the values that one place can hold. Objects and functions
/// from one node with different holders join to that node without a holder.
pub fn join_value<'a>(acc: &mut Option<Value<'a>>, value: Value<'a>) {
  let Some(prev) = acc else {
    *acc = Some(value);
    return;
  };
  if prev.same(&value) {
    return;
  }
  *acc = Some(match (&*prev, value) {
    (Value::Object(a), Value::Object(b)) if a.key() == b.key() => {
      Value::Object(ObjValue { holder: None, ..b })
    }
    (Value::Array(a), Value::Array(b)) if a.key() == b.key() => {
      Value::Array(ArrValue { holder: None, ..b })
    }
    (Value::Function(a), Value::Function(b)) if a.key() == b.key() => {
      Value::Function(FnValue { holder: None, ..b })
    }
    (Value::Class(a), Value::Class(b)) if a.key() == b.key() => {
      Value::Class(ClassValue { holder: None, ..b })
    }
    _ => Value::Unknown,
  });
}

/// Where a binding is inside a destructuring pattern.
pub enum PatternPath {
  NotFound,
  /// The keys from the root of the pattern to the binding.
  Keys(Vec<CompactStr>),
  /// The path goes through a part that the analysis does not model: an array pattern, a computed
  /// key, a default value, or a rest element.
  Unknown,
}

/// The path of `symbol_id` in `pattern`.
pub fn pattern_path(pattern: &BindingPattern, symbol_id: SymbolId) -> PatternPath {
  match pattern {
    BindingPattern::BindingIdentifier(id) => {
      if id.symbol_id.get() == Some(symbol_id) {
        PatternPath::Keys(Vec::new())
      } else {
        PatternPath::NotFound
      }
    }
    BindingPattern::ObjectPattern(obj) => {
      for prop in &obj.properties {
        match pattern_path(&prop.value, symbol_id) {
          PatternPath::NotFound => {}
          PatternPath::Keys(mut keys) => {
            return match static_binding_key(prop) {
              Some(key) => {
                keys.insert(0, key);
                PatternPath::Keys(keys)
              }
              None => PatternPath::Unknown,
            };
          }
          PatternPath::Unknown => return PatternPath::Unknown,
        }
      }
      match &obj.rest {
        Some(rest) if !matches!(pattern_path(&rest.argument, symbol_id), PatternPath::NotFound) => {
          PatternPath::Unknown
        }
        _ => PatternPath::NotFound,
      }
    }
    BindingPattern::ArrayPattern(arr) => {
      let found = arr
        .elements
        .iter()
        .flatten()
        .chain(arr.rest.as_ref().map(|rest| &rest.argument))
        .any(|element| !matches!(pattern_path(element, symbol_id), PatternPath::NotFound));
      if found { PatternPath::Unknown } else { PatternPath::NotFound }
    }
    BindingPattern::AssignmentPattern(assign) => match pattern_path(&assign.left, symbol_id) {
      PatternPath::NotFound => PatternPath::NotFound,
      _ => PatternPath::Unknown,
    },
  }
}

/// The key of a property of an object pattern, when it is not computed.
pub fn static_binding_key(prop: &BindingProperty) -> Option<CompactStr> {
  if prop.computed {
    return None;
  }
  static_key_name(&prop.key)
}

/// The name of a property key that is not computed.
pub fn static_key_name(key: &PropertyKey) -> Option<CompactStr> {
  match key {
    // `static_name` formats a number with Rust's rules, which differ from JavaScript's for
    // exponents (`1e21` is `1e+21`), and gives a BigInt in its source form.
    PropertyKey::NumericLiteral(lit) => number_key(lit.value),
    PropertyKey::BigIntLiteral(_) => None,
    key => key.static_name().and_then(|name| name.as_str().map(CompactStr::new)),
  }
}

/// The property key that a number converts to, for numbers that print the same in Rust and in
/// JavaScript.
#[expect(clippy::cast_possible_truncation)]
fn number_key(value: f64) -> Option<CompactStr> {
  (value.fract() == 0.0 && value.abs() < 1e15).then(|| CompactStr::new(&(value as i64).to_string()))
}

/// `__proto__: value` in an object literal sets the prototype. It is not an own property.
pub fn is_proto_setter(prop: &ObjectProperty) -> bool {
  prop.kind == PropertyKind::Init
    && !prop.computed
    && !prop.shorthand
    && !prop.method
    && prop.key.is_specific_static_name("__proto__")
}

pub fn has_proto_setter(obj: &ObjectExpression) -> bool {
  obj
    .properties
    .iter()
    .any(|prop| matches!(prop, ObjectPropertyKind::ObjectProperty(prop) if is_proto_setter(prop)))
}

/// The own property of an object literal at one key.
pub enum Slot<'a> {
  Data(&'a ObjectProperty<'a>),
  Accessor {
    get: Option<&'a ObjectProperty<'a>>,
    set: Option<&'a ObjectProperty<'a>>,
  },
  /// The literal does not define the key, so a read finds the prototype.
  Missing,
  /// A spread or a computed key that the analysis does not know can define the key.
  Unknown,
}

/// The prototype of an object literal.
pub enum Proto<'a> {
  /// `Object.prototype`. Its only accessor is `__proto__`, which runs no user code.
  Default,
  Null,
  Object(ObjValue<'a>),
  Unknown,
}

/// The function of a getter or a setter of an object literal. It gets the object as `this`, so
/// the analysis models its calls only when no method of the literal reads `this`. Then the call
/// does not depend on the object.
pub fn accessor_fn<'a>(obj: &ObjValue<'a>, prop: &'a ObjectProperty<'a>) -> Option<FnValue<'a>> {
  match &prop.value {
    Expression::FunctionExpression(func) => Some(FnValue {
      module: obj.site.module,
      stmt: obj.site.stmt,
      node: FnNode::Function(func),
      holder: obj.holder,
    }),
    _ => None,
  }
}

/// How an identifier resolves in an analyzed module.
pub enum Resolved {
  /// An unresolved reference, which reads a global.
  Global,
  /// A binding in the root scope of the module.
  Root(SymbolId),
  /// A binding in an inner scope.
  Local(SymbolId),
  None,
}

#[derive(PartialEq, Eq)]
enum Flow {
  Normal,
  Abrupt,
}

impl<'a> Analyzer<'_, 'a> {
  pub fn resolve(&self, module: ModuleIdx, ident: &IdentifierReference) -> Resolved {
    let Some(facts) = self.env.module(module) else {
      return Resolved::None;
    };
    let Some(ref_id) = ident.reference_id.get() else {
      return Resolved::None;
    };
    match facts.scopes.symbol_id_for(ref_id) {
      Some(symbol_id) => {
        if self.env.symbols.is_declared_in_root_scope((module, symbol_id).into()) {
          Resolved::Root(symbol_id)
        } else {
          Resolved::Local(symbol_id)
        }
      }
      None if facts.scopes.is_unresolved(ref_id) => Resolved::Global,
      None => Resolved::None,
    }
  }

  pub fn value(&mut self, expr: &'a Expression<'a>, site: Site) -> Value<'a> {
    if self.state.depth >= MAX_DEPTH {
      return Value::Unknown;
    }
    self.state.depth += 1;
    let value = self.value_inner(expr, site);
    self.state.depth -= 1;
    value
  }

  fn value_inner(&mut self, expr: &'a Expression<'a>, site: Site) -> Value<'a> {
    match expr {
      Expression::BooleanLiteral(lit) => Value::Lit(Lit::Bool(lit.value)),
      Expression::NullLiteral(_) => Value::Lit(Lit::Null),
      Expression::NumericLiteral(lit) => Value::Lit(Lit::Number(lit.value)),
      Expression::StringLiteral(lit) => {
        lit.value.as_str().map_or(Value::Unknown, |value| Value::Lit(Lit::Str(value.into())))
      }
      Expression::TemplateLiteral(template) => template
        .single_quasi()
        .and_then(|quasi| quasi.as_str().map(CompactStr::new))
        .map_or(Value::Unknown, |value| Value::Lit(Lit::Str(value))),
      Expression::Identifier(ident) => self.ident_value(ident, site),
      Expression::ParenthesizedExpression(paren) => self.value(&paren.expression, site),
      Expression::SequenceExpression(seq) => {
        seq.expressions.last().map_or(Value::Unknown, |last| self.value(last, site))
      }
      Expression::FunctionExpression(func) => Value::Function(FnValue {
        module: site.module,
        stmt: site.stmt,
        node: FnNode::Function(func),
        holder: None,
      }),
      Expression::ArrowFunctionExpression(arrow) => Value::Function(FnValue {
        module: site.module,
        stmt: site.stmt,
        node: FnNode::Arrow(arrow),
        holder: None,
      }),
      Expression::ObjectExpression(obj) => {
        Value::Object(ObjValue { site, node: obj, holder: None })
      }
      Expression::ArrayExpression(arr) => Value::Array(ArrValue { site, node: arr, holder: None }),
      Expression::ClassExpression(class) => {
        Value::Class(ClassValue { site, node: class, holder: None })
      }
      Expression::UnaryExpression(unary) => self.unary_value(unary, site),
      Expression::BinaryExpression(binary) => self.binary_value(binary, site),
      Expression::LogicalExpression(logical) => self.logical_value(logical, site),
      Expression::ConditionalExpression(cond) => self.conditional_value(cond, site),
      Expression::CallExpression(call) => self.call_value(call, site),
      Expression::ChainExpression(chain) => match &chain.expression {
        ChainElement::CallExpression(call) => self.call_value(call, site),
        ChainElement::TSNonNullExpression(inner) => self.value(&inner.expression, site),
        element => self.member_expr_value(element.to_member_expression(), site),
      },
      oxc::ast::match_member_expression!(Expression) => {
        self.member_expr_value(expr.to_member_expression(), site)
      }
      _ => Value::Unknown,
    }
  }

  fn member_expr_value(&mut self, member: &'a MemberExpression<'a>, site: Site) -> Value<'a> {
    let object = self.value(member.object(), site);
    // A nullish object ends the whole chain, which the parts of the chain cannot express.
    if member.optional() && object.nullish() != Some(false) {
      return Value::Unknown;
    }
    let key = self.member_key(member, site);
    self.member_value(object, key.as_deref(), site)
  }

  /// The value of a call. A known function is never nullish, so an optional call of it runs.
  fn call_value(&mut self, call: &'a CallExpression<'a>, site: Site) -> Value<'a> {
    if self.is_global_symbol_call(call, site.module) {
      return Value::Symbol;
    }
    match self.value(&call.callee, site) {
      Value::Function(func) => {
        self.record_call(&func, call, site);
        self.return_value(func)
      }
      _ => Value::Unknown,
    }
  }

  /// Whether `call` calls the global `Symbol`.
  pub fn is_global_symbol_call(&self, call: &CallExpression<'a>, module: ModuleIdx) -> bool {
    matches!(
      call.callee.without_parentheses(),
      Expression::Identifier(ident)
        if ident.name == "Symbol" && matches!(self.resolve(module, ident), Resolved::Global)
    )
  }

  /// The value of an initializer of a top-level binding (`holder`), or of a property value of an
  /// object literal. An object literal or a function directly in an initializer runs one time, so
  /// it gets the holder.
  pub fn init_value(
    &mut self,
    expr: &'a Expression<'a>,
    site: Site,
    holder: Option<SymbolRef>,
  ) -> Value<'a> {
    match expr.without_parentheses() {
      Expression::ObjectExpression(obj) => Value::Object(ObjValue { site, node: obj, holder }),
      Expression::ArrayExpression(arr) => Value::Array(ArrValue { site, node: arr, holder }),
      Expression::ClassExpression(class) => Value::Class(ClassValue { site, node: class, holder }),
      Expression::FunctionExpression(func) => Value::Function(FnValue {
        module: site.module,
        stmt: site.stmt,
        node: FnNode::Function(func),
        holder,
      }),
      Expression::ArrowFunctionExpression(arrow) => Value::Function(FnValue {
        module: site.module,
        stmt: site.stmt,
        node: FnNode::Arrow(arrow),
        holder,
      }),
      expr => self.value(expr, site),
    }
  }

  pub fn ident_value(&mut self, ident: &IdentifierReference<'a>, site: Site) -> Value<'a> {
    match self.resolve(site.module, ident) {
      Resolved::Global => match ident.name.as_str() {
        "undefined" => Value::Lit(Lit::Undefined),
        "NaN" => Value::Lit(Lit::Number(f64::NAN)),
        "Infinity" => Value::Lit(Lit::Number(f64::INFINITY)),
        _ => Value::Unknown,
      },
      Resolved::Root(symbol_id) => {
        let canonical = self.env.canonical(site.module, symbol_id);
        self.binding_value(site, canonical)
      }
      Resolved::Local(symbol_id) => self.local_value((site.module, symbol_id)),
      Resolved::None => Value::Unknown,
    }
  }

  /// The value of a binding inside a function: a parameter, or a function declaration. Other
  /// bindings inside functions are unknown.
  fn local_value(&mut self, key: LocalKey) -> Value<'a> {
    if self.state.is_reassigned(SymbolRef::from(key)) {
      return Value::Unknown;
    }
    if let Some(param) = self.state.params.get(&key).copied() {
      return self.param_value(key, param);
    }
    self.state.local_fns.get(&key).map_or(Value::Unknown, |func| Value::Function(*func))
  }

  /// The join of the arguments that the calls of the function pass to the parameter. Each call
  /// that can run is in `State::calls` by the end of the fixpoint, unless unknown code can call
  /// the function. Then the parameter is unknown.
  fn param_value(&mut self, key: LocalKey, param: ParamInfo<'a>) -> Value<'a> {
    if param.rest || param.has_default {
      return Value::Unknown;
    }
    if let Some(value) = self.state.param_values.get(&key) {
      return value.clone();
    }
    if !self.state.active_params.insert(key) {
      return Value::Unknown;
    }
    let mut value = self.arg_join(param.module, param.func, param.index);
    self.state.active_params.remove(&key);
    if let Some(pattern) = param.pattern {
      // The destructuring runs in the function. The site is only used for init safety, and a
      // site in no statement is never init safe.
      let site = Site { module: param.module, stmt: StmtInfoIdx::from_usize(0), in_function: true };
      value = self.pattern_binding_value(pattern, key.1, value, site);
    }
    self.state.param_values.insert(key, value.clone());
    value
  }

  /// The join of the values that the calls of a function pass at `index`. Unknown when unknown
  /// code can call the function.
  pub fn arg_join(&mut self, module: ModuleIdx, node: FnNode<'a>, index: usize) -> Value<'a> {
    let func = FnValue { module, stmt: StmtInfoIdx::from_usize(0), node, holder: None };
    if self.is_fn_escaped(module, node) || self.fn_uses_arguments(&func) {
      return Value::Unknown;
    }
    let records = self
      .state
      .calls
      .get(&func.key())
      .map(|calls| calls.values().copied().collect::<Vec<_>>())
      .unwrap_or_default();
    let mut acc = None;
    for record in records {
      let value = self.arg_value(&record, index);
      join_value(&mut acc, value);
      if matches!(acc, Some(Value::Unknown)) {
        break;
      }
    }
    acc.unwrap_or(Value::Unknown)
  }

  /// The value that a call passes at `index`.
  pub fn arg_value(&mut self, record: &CallRecord<'a>, index: usize) -> Value<'a> {
    for (i, arg) in record.args.iter().enumerate() {
      match arg {
        Argument::SpreadElement(_) => return Value::Unknown,
        arg if i == index => return self.value(arg.to_expression(), record.site),
        _ => {}
      }
    }
    Value::Lit(Lit::Undefined)
  }

  /// The value of the binding `symbol_id` in `pattern`, when `source` is the destructured value.
  fn pattern_binding_value(
    &mut self,
    pattern: &BindingPattern<'a>,
    symbol_id: SymbolId,
    source: Value<'a>,
    site: Site,
  ) -> Value<'a> {
    let PatternPath::Keys(keys) = pattern_path(pattern, symbol_id) else {
      return Value::Unknown;
    };
    let mut value = source;
    for key in keys {
      value = self.member_value(value, Some(&key), site);
    }
    value
  }

  /// Whether unknown code can call the function, or read its parameters or what it returns.
  pub fn is_fn_escaped(&self, module: ModuleIdx, node: FnNode<'a>) -> bool {
    if self.state.entity((module, node.node_id())).is_some_and(|entity| entity.escaped) {
      return true;
    }
    let FnNode::Function(func) = node else {
      return false;
    };
    if !func.is_function_declaration() {
      return false;
    }
    let Some(holder) = self.fn_decl_holder(module, func) else {
      return true;
    };
    self.state.bindings.get(&holder).is_some_and(|facts| {
      facts.demand.whole || CALL_KEYS.iter().any(|key| facts.demand.children.contains_key(*key))
    })
  }

  /// Whether a demand on a function value lets code call it in a way that the analysis does
  /// not see.
  pub fn is_call_revealing(demand: &super::facts::PathTree) -> bool {
    demand.whole || CALL_KEYS.iter().any(|key| demand.children.contains_key(*key))
  }

  /// The binding that a function declaration creates.
  fn fn_decl_holder(&self, module: ModuleIdx, func: &Function<'a>) -> Option<SymbolRef> {
    let symbol_id = match func.id.as_ref() {
      Some(id) => id.symbol_id.get()?,
      None => self.env.module(module)?.module.default_export_ref.symbol,
    };
    Some(self.symbol_key(module, symbol_id))
  }

  /// The key of the facts of a binding: the canonical ref for a top-level binding, and its own
  /// ref for a binding inside a function.
  pub fn symbol_key(&self, module: ModuleIdx, symbol_id: SymbolId) -> SymbolRef {
    let symbol_ref = SymbolRef::from((module, symbol_id));
    if self.env.symbols.is_declared_in_root_scope(symbol_ref) {
      self.env.canonical(module, symbol_id)
    } else {
      symbol_ref
    }
  }

  /// Record the parameters of a function, so that reads of them find their values.
  pub fn register_params(&mut self, func: &FnValue<'a>) {
    if !self.state.registered_fns.insert(func.key()) {
      return;
    }
    let params = func.node.params();
    let mut add = |pattern: &'a BindingPattern<'a>, index: usize, rest: bool, has_default: bool| {
      let nested = !matches!(pattern, BindingPattern::BindingIdentifier(_));
      for id in pattern.get_binding_identifiers() {
        if let Some(symbol_id) = id.symbol_id.get() {
          self.state.params.insert(
            (func.module, symbol_id),
            ParamInfo {
              module: func.module,
              func: func.node,
              index,
              pattern: nested.then_some(pattern),
              rest,
              has_default,
            },
          );
        }
      }
    };
    for (index, item) in params.items.iter().enumerate() {
      add(&item.pattern, index, false, item.initializer.is_some());
    }
    if let Some(rest) = &params.rest {
      add(&rest.rest.argument, params.items.len(), true, false);
    }
  }

  /// Record the function declarations of a statement list inside a function or a block. They
  /// are initialized when the list starts.
  pub fn register_local_fns(&mut self, stmts: &'a [Statement<'a>], site: Site) {
    let Some(facts) = self.env.module(site.module) else {
      return;
    };
    let scoping = facts.scopes.scoping();
    for stmt in stmts {
      if let Statement::FunctionDeclaration(func) = stmt
        && let Some(symbol_id) = func.id.as_ref().and_then(|id| id.symbol_id.get())
        && !self.env.symbols.is_declared_in_root_scope((site.module, symbol_id).into())
        && scoping.symbol_redeclarations(symbol_id).is_empty()
      {
        let key = (site.module, symbol_id);
        self.state.local_fns.entry(key).or_insert(FnValue {
          module: site.module,
          stmt: site.stmt,
          node: FnNode::Function(func),
          holder: Some(SymbolRef::from(key)),
        });
      }
    }
  }

  /// The value of a binding at `site`. It is the value of the initializer only when nothing
  /// writes the binding and `site` cannot see it uninitialized.
  pub fn binding_value(&mut self, site: Site, canonical: SymbolRef) -> Value<'a> {
    if let Some(module) = self.env.namespace_module(canonical) {
      return Value::Namespace(module);
    }
    let Some(decl) = self.env.decl(canonical) else {
      return Value::Unknown;
    };
    if !decl.unique
      || self.state.is_reassigned(canonical)
      || !self.env.is_init_safe(site, canonical, &decl)
    {
      return Value::Unknown;
    }
    match decl.init {
      Init::Function(func) => Value::Function(FnValue {
        module: canonical.owner,
        stmt: decl.stmt,
        node: FnNode::Function(func),
        holder: Some(canonical),
      }),
      Init::Class(class) => Value::Class(ClassValue {
        site: Site { module: canonical.owner, stmt: decl.stmt, in_function: false },
        node: class,
        holder: Some(canonical),
      }),
      Init::None => match decl.kind {
        DeclKind::Var | DeclKind::Let => Value::Lit(Lit::Undefined),
        _ => Value::Unknown,
      },
      Init::Expr(expr) => {
        if !matches!(decl.kind, DeclKind::Var | DeclKind::Let | DeclKind::Const | DeclKind::Default)
          || !self.state.active_bindings.insert(canonical)
        {
          return Value::Unknown;
        }
        let decl_site = Site { module: canonical.owner, stmt: decl.stmt, in_function: false };
        let value = self.init_value(expr, decl_site, Some(canonical));
        self.state.active_bindings.remove(&canonical);
        value
      }
      Init::Pattern(expr, pattern) => {
        if !self.state.active_bindings.insert(canonical) {
          return Value::Unknown;
        }
        let decl_site = Site { module: canonical.owner, stmt: decl.stmt, in_function: false };
        let source = self.value(expr, decl_site);
        let value = self.pattern_binding_value(pattern, canonical.symbol, source, decl_site);
        self.state.active_bindings.remove(&canonical);
        value
      }
    }
  }

  pub fn member_key(&mut self, member: &'a MemberExpression<'a>, site: Site) -> Option<CompactStr> {
    match member {
      MemberExpression::StaticMemberExpression(member) => {
        Some(CompactStr::new(member.property.name.as_str()))
      }
      MemberExpression::ComputedMemberExpression(member) => {
        self.computed_key(&member.expression, site)
      }
      MemberExpression::PrivateFieldExpression(_) => None,
    }
  }

  /// The property key that `expr` converts to, when its value is a known primitive.
  pub fn computed_key(&mut self, expr: &'a Expression<'a>, site: Site) -> Option<CompactStr> {
    match self.value(expr, site) {
      Value::Lit(Lit::Str(value)) => Some(value),
      Value::Lit(Lit::Number(value)) => number_key(value),
      Value::Lit(Lit::Bool(value)) => Some(CompactStr::new(if value { "true" } else { "false" })),
      Value::Lit(Lit::Null) => Some(CompactStr::new("null")),
      Value::Lit(Lit::Undefined) => Some(CompactStr::new("undefined")),
      _ => None,
    }
  }

  pub fn property_key(&mut self, prop: &'a ObjectProperty<'a>, site: Site) -> Option<CompactStr> {
    self.key_name(&prop.key, prop.computed, site)
  }

  pub fn binding_key(&mut self, prop: &'a BindingProperty<'a>, site: Site) -> Option<CompactStr> {
    self.key_name(&prop.key, prop.computed, site)
  }

  fn key_name(
    &mut self,
    key: &'a PropertyKey<'a>,
    computed: bool,
    site: Site,
  ) -> Option<CompactStr> {
    if computed {
      return key.as_expression().and_then(|key| self.computed_key(key, site));
    }
    static_key_name(key)
  }

  pub fn member_value(&mut self, object: Value<'a>, key: Option<&str>, site: Site) -> Value<'a> {
    let Some(key) = key else {
      return Value::Unknown;
    };
    match object {
      Value::Object(obj) => self.object_property_value(obj, key),
      Value::Array(arr) => self.array_property_value(arr, key),
      Value::Class(class) => self.class_static_value(class, key),
      Value::Namespace(module) => match self.env.namespace_export(module, key) {
        Some(export) => self.binding_value(site, export),
        None if self.env.namespace_has(module, key) == Some(false) => Value::Lit(Lit::Undefined),
        None => Value::Unknown,
      },
      Value::Lit(Lit::Str(value)) if key == "length" => {
        #[expect(clippy::cast_precision_loss)]
        let length = value.encode_utf16().count() as f64;
        Value::Lit(Lit::Number(length))
      }
      _ => Value::Unknown,
    }
  }

  fn object_property_value(&mut self, obj: ObjValue<'a>, key: &str) -> Value<'a> {
    if key == "__proto__" || self.is_object_escaped(&obj) || self.is_key_deopted(&obj, key) {
      return Value::Unknown;
    }
    match self.own_slot(&obj, key) {
      Slot::Data(prop) => self.init_value(&prop.value, obj.site, obj.holder),
      // A getter that does not read `this` returns the same value for each object.
      Slot::Accessor { get: Some(getter), .. } => match accessor_fn(&obj, getter) {
        Some(func) => self.return_value(func),
        None => Value::Unknown,
      },
      Slot::Accessor { get: None, .. } => Value::Lit(Lit::Undefined),
      Slot::Missing => match self.proto_of(&obj) {
        // Code can add a key to `Object.prototype`. Like rollup, the analysis assumes that the
        // key is not an array index. See internal-docs/advanced-tree-shaking/design.md.
        Proto::Default if array_index(key).is_none() => Value::Unknown,
        Proto::Default | Proto::Null => Value::Lit(Lit::Undefined),
        Proto::Object(proto) => {
          if self.state.depth >= MAX_DEPTH {
            return Value::Unknown;
          }
          self.state.depth += 1;
          let value = self.object_property_value(proto, key);
          self.state.depth -= 1;
          value
        }
        Proto::Unknown => Value::Unknown,
      },
      Slot::Unknown => Value::Unknown,
    }
  }

  /// An element or the length of an array literal. Other keys are inherited methods.
  fn array_property_value(&mut self, arr: ArrValue<'a>, key: &str) -> Value<'a> {
    if self.is_array_escaped(&arr)
      || self.state.entity(arr.key()).is_some_and(|entity| entity.all_keys_deopted)
    {
      return Value::Unknown;
    }
    let elements = &arr.node.elements;
    let first_spread = elements
      .iter()
      .position(|element| matches!(element, ArrayExpressionElement::SpreadElement(_)));
    if key == "length" {
      #[expect(clippy::cast_precision_loss)]
      return match first_spread {
        Some(_) => Value::Unknown,
        None => Value::Lit(Lit::Number(elements.len() as f64)),
      };
    }
    let Some(index) = array_index(key) else {
      return Value::Unknown;
    };
    if first_spread.is_some_and(|spread| spread <= index) {
      return Value::Unknown;
    }
    match elements.get(index) {
      // A hole and an index past the end find no element in the prototype chain.
      None | Some(ArrayExpressionElement::Elision(_)) => Value::Lit(Lit::Undefined),
      Some(element) => self.init_value(element.to_expression(), arr.site, arr.holder),
    }
  }

  /// Whether unknown code can see the array.
  pub fn is_array_escaped(&self, arr: &ArrValue<'a>) -> bool {
    self.state.all_objects_deopted
      || self.state.entity(arr.key()).is_some_and(|entity| entity.escaped)
      || arr.holder.is_some_and(|holder| self.state.is_escaped(holder))
  }

  /// Whether a visited write or `delete` can have changed the property `key` of the objects of
  /// the literal.
  pub fn is_key_deopted(&self, obj: &ObjValue<'a>, key: &str) -> bool {
    self
      .state
      .entity(obj.key())
      .is_some_and(|entity| entity.all_keys_deopted || entity.deopted_keys.contains(key))
  }

  /// Whether a visited `delete` can remove the property `key` of the objects of the literal. A
  /// `delete` with an unknown key makes the objects escape.
  pub fn is_key_deleted(&self, obj: &ObjValue<'a>, key: &str) -> bool {
    self
      .state
      .entity(obj.key())
      .is_some_and(|entity| entity.all_keys_deopted || entity.deleted_keys.contains(key))
  }

  /// The own property `key` that the literal defines. The last definition of a key wins. A
  /// getter and a setter of one key make one accessor property, and a data property between
  /// them splits them.
  pub fn own_slot(&mut self, obj: &ObjValue<'a>, key: &str) -> Slot<'a> {
    let mut get: Option<&'a ObjectProperty<'a>> = None;
    let mut set: Option<&'a ObjectProperty<'a>> = None;
    for prop in obj.node.properties.iter().rev() {
      let ObjectPropertyKind::ObjectProperty(prop) = prop else {
        // A spread can define the key as a data property.
        return Slot::Unknown;
      };
      if is_proto_setter(prop) {
        continue;
      }
      match self.property_key(prop, obj.site) {
        Some(name) if name == key => {}
        Some(_) => continue,
        None => return Slot::Unknown,
      }
      match prop.kind {
        PropertyKind::Init if get.is_none() && set.is_none() => return Slot::Data(prop),
        PropertyKind::Init => break,
        PropertyKind::Get => {
          get.get_or_insert(prop);
        }
        PropertyKind::Set => {
          set.get_or_insert(prop);
        }
      }
    }
    if get.is_none() && set.is_none() { Slot::Missing } else { Slot::Accessor { get, set } }
  }

  /// The prototype that an object literal sets with `__proto__: value`.
  pub fn proto_of(&mut self, obj: &ObjValue<'a>) -> Proto<'a> {
    let proto = obj.node.properties.iter().find_map(|prop| match prop {
      ObjectPropertyKind::ObjectProperty(prop) if is_proto_setter(prop) => Some(&prop.value),
      _ => None,
    });
    let Some(proto) = proto else {
      return Proto::Default;
    };
    match self.init_value(proto, obj.site, obj.holder) {
      Value::Lit(Lit::Null) => Proto::Null,
      Value::Object(proto) => Proto::Object(proto),
      _ => Proto::Unknown,
    }
  }

  /// Whether unknown code can see the object. Then every property of it is unknown.
  pub fn is_object_escaped(&mut self, obj: &ObjValue<'a>) -> bool {
    self.state.all_objects_deopted
      || self.state.entity(obj.key()).is_some_and(|entity| entity.escaped)
      || obj.holder.is_some_and(|holder| self.state.is_escaped(holder))
      || self.object_uses_this(obj.site.module, obj.node)
  }

  /// Whether a method of the object reads `this` or `super`, which can change the object.
  pub fn object_uses_this(&mut self, module: ModuleIdx, obj: &'a ObjectExpression<'a>) -> bool {
    *self
      .state
      .syntax
      .uses_this
      .entry((module, obj.node_id()))
      .or_insert_with(|| collect::object_uses_this(obj))
  }

  pub fn fn_uses_this(&mut self, func: &FnValue<'a>) -> bool {
    let FnNode::Function(node) = func.node else {
      return false;
    };
    *self
      .state
      .syntax
      .uses_this
      .entry(func.key())
      .or_insert_with(|| collect::function_uses_this(node))
  }

  pub fn fn_uses_arguments(&mut self, func: &FnValue<'a>) -> bool {
    let FnNode::Function(node) = func.node else {
      return false;
    };
    *self
      .state
      .syntax
      .uses_arguments
      .entry(func.key())
      .or_insert_with(|| collect::function_uses_arguments(node))
  }

  /// The value that every call of `func` returns, if it is the same for every call.
  pub fn return_value(&mut self, func: FnValue<'a>) -> Value<'a> {
    let key = func.key();
    if let Some(value) = self.state.return_values.get(&key) {
      return value.clone();
    }
    if func.node.is_async()
      || func.node.is_generator()
      || self.state.depth >= MAX_DEPTH
      || !self.state.active_returns.insert(key)
    {
      return Value::Unknown;
    }
    self.state.depth += 1;
    self.register_params(&func);
    let site = func.body_site();
    let value = match func.node.expression_body() {
      Some(expr) => self.value(expr, site),
      None => {
        let mut acc = None;
        if self.collect_returns(func.node.body_statements(), site, &mut acc) == Flow::Normal {
          merge_value(&mut acc, Value::Lit(Lit::Undefined));
        }
        acc.unwrap_or(Value::Unknown)
      }
    };
    self.state.depth -= 1;
    self.state.active_returns.remove(&key);
    self.state.return_values.insert(key, value.clone());
    value
  }

  fn collect_returns(
    &mut self,
    stmts: &'a [Statement<'a>],
    site: Site,
    acc: &mut Option<Value<'a>>,
  ) -> Flow {
    self.register_local_fns(stmts, site);
    for stmt in stmts {
      if self.collect_returns_in(stmt, site, acc) == Flow::Abrupt {
        return Flow::Abrupt;
      }
    }
    Flow::Normal
  }

  fn collect_returns_in(
    &mut self,
    stmt: &'a Statement<'a>,
    site: Site,
    acc: &mut Option<Value<'a>>,
  ) -> Flow {
    match stmt {
      Statement::ReturnStatement(ret) => {
        let value =
          ret.argument.as_ref().map_or(Value::Lit(Lit::Undefined), |arg| self.value(arg, site));
        merge_value(acc, value);
        Flow::Abrupt
      }
      Statement::ThrowStatement(_) => Flow::Abrupt,
      Statement::BlockStatement(block) => self.collect_returns(&block.body, site, acc),
      Statement::IfStatement(if_stmt) => match self.value(&if_stmt.test, site).truthy() {
        Some(true) => self.collect_returns_in(&if_stmt.consequent, site, acc),
        Some(false) => if_stmt
          .alternate
          .as_ref()
          .map_or(Flow::Normal, |alt| self.collect_returns_in(alt, site, acc)),
        None => {
          let consequent = self.collect_returns_in(&if_stmt.consequent, site, acc);
          let alternate = if_stmt
            .alternate
            .as_ref()
            .map_or(Flow::Normal, |alt| self.collect_returns_in(alt, site, acc));
          if consequent == Flow::Abrupt && alternate == Flow::Abrupt {
            Flow::Abrupt
          } else {
            Flow::Normal
          }
        }
      },
      stmt if collect::contains_return(stmt) => {
        // A `return` inside a loop, `switch`, `try`, or label: give up.
        *acc = Some(Value::Unknown);
        Flow::Normal
      }
      _ => Flow::Normal,
    }
  }

  fn unary_value(&mut self, unary: &'a UnaryExpression<'a>, site: Site) -> Value<'a> {
    match unary.operator {
      UnaryOperator::Void => Value::Lit(Lit::Undefined),
      UnaryOperator::LogicalNot => self
        .value(&unary.argument, site)
        .truthy()
        .map_or(Value::Unknown, |truthy| Value::Lit(Lit::Bool(!truthy))),
      UnaryOperator::Typeof => match self.value(&unary.argument, site) {
        Value::Lit(lit) => Value::Lit(Lit::Str(CompactStr::new(lit.type_of()))),
        Value::Object(_) | Value::Array(_) | Value::Namespace(_) => {
          Value::Lit(Lit::Str(CompactStr::new("object")))
        }
        Value::Function(_) | Value::Class(_) => Value::Lit(Lit::Str(CompactStr::new("function"))),
        Value::Symbol => Value::Lit(Lit::Str(CompactStr::new("symbol"))),
        Value::Unknown => Value::Unknown,
      },
      UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => {
        let Value::Lit(lit) = self.value(&unary.argument, site) else {
          return Value::Unknown;
        };
        match lit.to_number() {
          Some(number) if unary.operator == UnaryOperator::UnaryNegation => {
            Value::Lit(Lit::Number(-number))
          }
          Some(number) => Value::Lit(Lit::Number(number)),
          None => Value::Unknown,
        }
      }
      _ => Value::Unknown,
    }
  }

  fn binary_value(&mut self, binary: &'a BinaryExpression<'a>, site: Site) -> Value<'a> {
    if binary.operator == BinaryOperator::In {
      let key = self.computed_key(&binary.left, site);
      return match (key, self.value(&binary.right, site)) {
        (Some(key), Value::Namespace(module)) => self
          .env
          .namespace_has(module, &key)
          .map_or(Value::Unknown, |has| Value::Lit(Lit::Bool(has))),
        _ => Value::Unknown,
      };
    }
    let left = self.value(&binary.left, site);
    let right = self.value(&binary.right, site);
    let bool_value = match binary.operator {
      BinaryOperator::StrictEquality => strict_equals(&left, &right),
      BinaryOperator::StrictInequality => strict_equals(&left, &right).map(|equal| !equal),
      BinaryOperator::Equality | BinaryOperator::Inequality => {
        let equal = match (&left, &right) {
          (Value::Lit(a), Value::Lit(b)) => a.loose_eq(b),
          _ => None,
        };
        if binary.operator == BinaryOperator::Equality { equal } else { equal.map(|equal| !equal) }
      }
      BinaryOperator::LessThan
      | BinaryOperator::LessEqualThan
      | BinaryOperator::GreaterThan
      | BinaryOperator::GreaterEqualThan => match (&left, &right) {
        (Value::Lit(Lit::Number(a)), Value::Lit(Lit::Number(b))) => Some(match binary.operator {
          BinaryOperator::LessThan => a < b,
          BinaryOperator::LessEqualThan => a <= b,
          BinaryOperator::GreaterThan => a > b,
          _ => a >= b,
        }),
        _ => None,
      },
      BinaryOperator::Addition => {
        return match (left, right) {
          (Value::Lit(Lit::Number(a)), Value::Lit(Lit::Number(b))) => {
            Value::Lit(Lit::Number(a + b))
          }
          (Value::Lit(Lit::Str(a)), Value::Lit(Lit::Str(b))) => {
            Value::Lit(Lit::Str(CompactStr::new(&format!("{a}{b}"))))
          }
          _ => Value::Unknown,
        };
      }
      _ => None,
    };
    bool_value.map_or(Value::Unknown, |value| Value::Lit(Lit::Bool(value)))
  }

  fn logical_value(&mut self, logical: &'a LogicalExpression<'a>, site: Site) -> Value<'a> {
    let left = self.value(&logical.left, site);
    let left_decides = match logical.operator {
      LogicalOperator::And => left.truthy().map(|truthy| !truthy),
      LogicalOperator::Or => left.truthy(),
      LogicalOperator::Coalesce => left.nullish().map(|nullish| !nullish),
    };
    match left_decides {
      Some(true) => left,
      Some(false) => self.value(&logical.right, site),
      None => Value::Unknown,
    }
  }

  fn conditional_value(&mut self, cond: &'a ConditionalExpression<'a>, site: Site) -> Value<'a> {
    match self.value(&cond.test, site).truthy() {
      Some(true) => self.value(&cond.consequent, site),
      Some(false) => self.value(&cond.alternate, site),
      None => Value::Unknown,
    }
  }
}

fn merge_value<'a>(acc: &mut Option<Value<'a>>, value: Value<'a>) {
  match acc {
    None => *acc = Some(value),
    Some(prev) => {
      if !prev.same(&value) {
        *acc = Some(Value::Unknown);
      }
    }
  }
}
