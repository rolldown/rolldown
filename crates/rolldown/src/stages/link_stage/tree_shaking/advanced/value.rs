use oxc::{
  ast::ast::{
    ArrowFunctionExpression, BinaryExpression, BinaryOperator, ConditionalExpression, Expression,
    FormalParameters, Function, IdentifierReference, LogicalExpression, LogicalOperator,
    MemberExpression, ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKey,
    PropertyKind, Statement, UnaryExpression, UnaryOperator,
  },
  semantic::{NodeId, ScopeId, SymbolId},
};
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, StmtInfoIdx, SymbolRef};

use super::{
  Analyzer, collect,
  env::{DeclKind, Init, Site},
};

/// A deeper evaluation gives up and returns `Value::Unknown`.
pub const MAX_DEPTH: u32 = 64;

/// The property names of `Object.prototype`. A read of one of them on an object literal that
/// does not define it finds the inherited value.
const OBJECT_PROTOTYPE_KEYS: [&str; 12] = [
  "__defineGetter__",
  "__defineSetter__",
  "__lookupGetter__",
  "__lookupSetter__",
  "__proto__",
  "constructor",
  "hasOwnProperty",
  "isPrototypeOf",
  "propertyIsEnumerable",
  "toLocaleString",
  "toString",
  "valueOf",
];

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

#[derive(Debug, Clone)]
pub enum Value<'a> {
  Unknown,
  Lit(Lit),
  Object(ObjValue<'a>),
  Function(FnValue<'a>),
  /// The namespace object of an analyzed ES module.
  Namespace(ModuleIdx),
}

impl Value<'_> {
  pub fn truthy(&self) -> Option<bool> {
    match self {
      Value::Unknown => None,
      Value::Lit(lit) => Some(lit.truthy()),
      Value::Object(_) | Value::Function(_) | Value::Namespace(_) => Some(true),
    }
  }

  pub fn nullish(&self) -> Option<bool> {
    match self {
      Value::Unknown => None,
      Value::Lit(lit) => Some(matches!(lit, Lit::Undefined | Lit::Null)),
      Value::Object(_) | Value::Function(_) | Value::Namespace(_) => Some(false),
    }
  }

  fn same(&self, other: &Value) -> bool {
    match (self, other) {
      (Value::Lit(a), Value::Lit(b)) => a.same(b),
      (Value::Object(a), Value::Object(b)) => {
        a.key() == b.key() && a.holder.is_some() && a.holder == b.holder
      }
      (Value::Function(a), Value::Function(b)) => {
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
    (Value::Lit(_), Value::Object(_) | Value::Function(_) | Value::Namespace(_))
    | (Value::Object(_) | Value::Function(_) | Value::Namespace(_), Value::Lit(_)) => Some(false),
    (Value::Object(a), Value::Object(b)) if a.holder.is_some() && b.holder.is_some() => {
      Some(a.key() == b.key())
    }
    (Value::Namespace(a), Value::Namespace(b)) => Some(a == b),
    _ => None,
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
      Expression::UnaryExpression(unary) => self.unary_value(unary, site),
      Expression::BinaryExpression(binary) => self.binary_value(binary, site),
      Expression::LogicalExpression(logical) => self.logical_value(logical, site),
      Expression::ConditionalExpression(cond) => self.conditional_value(cond, site),
      Expression::CallExpression(call) => {
        if call.optional {
          return Value::Unknown;
        }
        match self.value(&call.callee, site) {
          Value::Function(func) => self.return_value(func),
          _ => Value::Unknown,
        }
      }
      oxc::ast::match_member_expression!(Expression) => {
        let member = expr.to_member_expression();
        if member.optional() {
          return Value::Unknown;
        }
        let key = self.member_key(member, site);
        let object = self.value(member.object(), site);
        self.member_value(object, key.as_deref(), site)
      }
      _ => Value::Unknown,
    }
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
      Resolved::Local(_) | Resolved::None => Value::Unknown,
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
    match &prop.key {
      // `static_name` formats a number with Rust's rules, which differ from JavaScript's for
      // exponents (`1e21` is `1e+21`), and gives a BigInt in its source form.
      PropertyKey::NumericLiteral(lit) => return number_key(lit.value),
      PropertyKey::BigIntLiteral(_) => return None,
      _ => {}
    }
    if let Some(name) = prop.key.static_name() {
      return name.as_str().map(CompactStr::new);
    }
    if prop.computed {
      return prop.key.as_expression().and_then(|key| self.computed_key(key, site));
    }
    None
  }

  pub fn member_value(&mut self, object: Value<'a>, key: Option<&str>, site: Site) -> Value<'a> {
    let Some(key) = key else {
      return Value::Unknown;
    };
    match object {
      Value::Object(obj) => self.object_property_value(obj, key),
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
    if key == "__proto__" || self.is_object_escaped(&obj) {
      return Value::Unknown;
    }
    if let Some(entity) = self.state.entity(obj.key())
      && (entity.all_keys_deopted || entity.deopted_keys.contains(key))
    {
      return Value::Unknown;
    }
    let site = obj.site;
    // The last definition of a key wins, unless a later spread or unknown key may replace it.
    for prop in obj.node.properties.iter().rev() {
      let ObjectPropertyKind::ObjectProperty(prop) = prop else {
        return Value::Unknown;
      };
      if is_proto_setter(prop) {
        continue;
      }
      match self.property_key(prop, site) {
        Some(name) if name == key => {
          if prop.kind != PropertyKind::Init {
            return Value::Unknown;
          }
          return self.init_value(&prop.value, site, obj.holder);
        }
        Some(_) => {}
        None => return Value::Unknown,
      }
    }
    if has_proto_setter(obj.node) || OBJECT_PROTOTYPE_KEYS.contains(&key) {
      return Value::Unknown;
    }
    Value::Lit(Lit::Undefined)
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
        Value::Object(_) | Value::Namespace(_) => Value::Lit(Lit::Str(CompactStr::new("object"))),
        Value::Function(_) => Value::Lit(Lit::Str(CompactStr::new("function"))),
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
