//! Classes and constructors: the static properties of a class, the prototype chain of the
//! objects that `new` creates, the effects of a class definition, and the effects of `new`.
//!
//! See internal-docs/advanced-tree-shaking/implementation.md.

use oxc::ast::ast::{
  Class, ClassElement, Expression, MethodDefinition, MethodDefinitionKind, NewExpression,
  PropertyDefinition, PropertyKey,
};
use oxc_str::CompactStr;
use rolldown_common::SymbolRef;

use super::{
  Analyzer, collect,
  effects::Cx,
  env::Site,
  facts::{CallArgs, NodeKey, PathTree},
  value::{
    ClassValue, FnNode, FnValue, Lit, MAX_DEPTH, Value, array_index, is_primitive_key,
    static_key_name,
  },
};

/// A constructor that the analysis knows: a class, or a function declaration.
#[derive(Debug, Clone, Copy)]
pub enum Ctor<'a> {
  Function(FnValue<'a>),
  Class(ClassValue<'a>),
}

impl Ctor<'_> {
  pub fn holder(&self) -> Option<SymbolRef> {
    match self {
      Ctor::Function(func) => func.holder,
      Ctor::Class(class) => class.holder,
    }
  }

  pub fn key(&self) -> NodeKey {
    match self {
      Ctor::Function(func) => func.key(),
      Ctor::Class(class) => class.key(),
    }
  }
}

/// The value of `this` in the code that an effect check analyzes.
#[derive(Debug, Clone, Copy)]
pub enum ThisValue<'a> {
  Unknown,
  /// The new object, while `new` of the constructor runs the fields and the body.
  Instance(Ctor<'a>),
  /// The class, while its definition runs its static fields and static blocks. No other code
  /// can see the class then.
  Class(ClassValue<'a>),
}

impl ThisValue<'_> {
  /// The constructor of the new object, which decides the result of an effect check of a
  /// constructor body.
  pub fn ctor_key(&self) -> Option<NodeKey> {
    match self {
      ThisValue::Instance(ctor) => Some(ctor.key()),
      ThisValue::Unknown | ThisValue::Class(_) => None,
    }
  }
}

/// The own property of a class, or of its prototype, at one key.
pub enum ClassSlot<'a> {
  /// A static field. The definition defines fields after all methods, so a field wins.
  Field(&'a PropertyDefinition<'a>),
  Method(&'a MethodDefinition<'a>),
  Accessor {
    get: Option<&'a MethodDefinition<'a>>,
    set: Option<&'a MethodDefinition<'a>>,
  },
  Missing,
  /// A computed key or an `accessor` property that the analysis does not know can define the
  /// key.
  Unknown,
}

/// What `extends` gives a class.
enum Parent<'a> {
  /// No `extends`, or `extends null`. The class inherits the builtins of `Function.prototype`,
  /// and its prototype inherits the builtins of `Object.prototype` or nothing.
  Builtin,
  Ctor(Ctor<'a>),
  Unknown,
}

enum KeyName {
  Known(CompactStr),
  /// A private name, which never matches a property key.
  Private,
  Unknown,
}

fn class_method_fn<'a>(class: &ClassValue<'a>, method: &'a MethodDefinition<'a>) -> FnValue<'a> {
  FnValue {
    module: class.site.module,
    stmt: class.site.stmt,
    node: FnNode::Function(&method.value),
    holder: class.holder,
  }
}

impl<'a> Analyzer<'_, 'a> {
  /// Whether `new` can call `func`: a function declaration or a function expression. `new` of
  /// an arrow function, a method, an async function, or a generator throws.
  pub fn is_es5_ctor(&mut self, func: &FnValue<'a>) -> bool {
    let FnNode::Function(node) = func.node else {
      return false;
    };
    if node.r#async || node.generator {
      return false;
    }
    if node.is_function_declaration() {
      return true;
    }
    let Some(facts) = self.env.module(func.module) else {
      return false;
    };
    !self
      .state
      .syntax
      .methods
      .entry(func.module)
      .or_insert_with(|| collect::method_functions(facts.program))
      .contains(&node.node_id())
  }

  /// Whether unknown code can see the function object, so it can define accessors on it.
  pub fn is_fn_object_escaped(&self, func: &FnValue<'a>) -> bool {
    self.state.all_objects_deopted
      || self.state.entity(func.key()).is_some_and(|entity| entity.escaped)
      || func.holder.is_some_and(|holder| self.state.is_escaped(holder))
      || self.is_proto_exposed(func.holder, func.key())
  }

  /// Whether included code can give the `prototype` object of a constructor to unknown code.
  /// The prototype leads back to the constructor through `constructor`.
  fn is_proto_exposed(&self, holder: Option<SymbolRef>, entity: NodeKey) -> bool {
    let exposed = |demand: &PathTree| demand.children.get("prototype").is_some_and(|sub| sub.whole);
    holder.is_some_and(|holder| {
      self.state.bindings.get(&holder).is_some_and(|facts| exposed(&facts.demand))
    }) || self.state.entity(entity).is_some_and(|entity| exposed(&entity.demand))
  }

  /// Whether unknown code can see the class, its prototype, or its objects, or code of the class
  /// can change the class in a way that the analysis does not track. Then every property of the
  /// class and of its prototype is unknown.
  pub fn is_class_escaped(&mut self, class: &ClassValue<'a>) -> bool {
    self.state.all_objects_deopted
      || self.state.entity(class.key()).is_some_and(|entity| entity.escaped)
      || class.holder.is_some_and(|holder| self.state.is_escaped(holder))
      || self.is_proto_exposed(class.holder, class.key())
      || *self
        .state
        .syntax
        .class_opaque
        .entry(class.key())
        .or_insert_with(|| collect::class_opaque(class.node))
  }

  /// Whether a visited write or `delete` can have changed the property `key` of the entity.
  pub fn is_entity_key_deopted(&self, entity: NodeKey, key: &str) -> bool {
    self
      .state
      .entity(entity)
      .is_some_and(|entity| entity.all_keys_deopted || entity.deopted_keys.contains(key))
  }

  /// Whether the analysis knows the prototype chain of the objects that `new` of `ctor`
  /// creates: no unknown code can have changed it.
  pub fn is_proto_known(&mut self, ctor: Ctor<'a>) -> bool {
    match ctor {
      Ctor::Function(func) => {
        !self.is_fn_object_escaped(&func) && !self.is_entity_key_deopted(func.key(), "prototype")
      }
      Ctor::Class(class) => !self.is_class_escaped(&class),
    }
  }

  fn class_key(&mut self, key: &'a PropertyKey<'a>, computed: bool, site: Site) -> KeyName {
    if computed {
      return key
        .as_expression()
        .and_then(|key| self.computed_key(key, site))
        .map_or(KeyName::Unknown, KeyName::Known);
    }
    if let PropertyKey::PrivateIdentifier(_) = key {
      return KeyName::Private;
    }
    static_key_name(key).map_or(KeyName::Unknown, KeyName::Known)
  }

  /// The own property `key` of the class (`is_static`) or of its prototype, as the definition
  /// creates it. The last definition of a key wins, and a getter and a setter of one key make
  /// one accessor property.
  pub fn class_slot(
    &mut self,
    class: &ClassValue<'a>,
    key: &str,
    is_static: bool,
  ) -> ClassSlot<'a> {
    let node: &'a Class<'a> = class.node;
    let elements = &node.body.body;
    if elements.iter().any(|element| matches!(element, ClassElement::AccessorProperty(_))) {
      return ClassSlot::Unknown;
    }
    if is_static {
      for element in elements.iter().rev() {
        if let ClassElement::PropertyDefinition(prop) = element
          && prop.r#static
        {
          match self.class_key(&prop.key, prop.computed, class.site) {
            KeyName::Known(name) if name == key => return ClassSlot::Field(prop),
            KeyName::Known(_) | KeyName::Private => {}
            KeyName::Unknown => return ClassSlot::Unknown,
          }
        }
      }
    }
    let mut get: Option<&'a MethodDefinition<'a>> = None;
    let mut set: Option<&'a MethodDefinition<'a>> = None;
    for element in elements.iter().rev() {
      let ClassElement::MethodDefinition(method) = element else {
        continue;
      };
      if method.r#static != is_static || method.kind == MethodDefinitionKind::Constructor {
        continue;
      }
      match self.class_key(&method.key, method.computed, class.site) {
        KeyName::Known(name) if name == key => {}
        KeyName::Known(_) | KeyName::Private => continue,
        KeyName::Unknown => return ClassSlot::Unknown,
      }
      match method.kind {
        MethodDefinitionKind::Get => {
          get.get_or_insert(method);
        }
        MethodDefinitionKind::Set => {
          set.get_or_insert(method);
        }
        _ if get.is_none() && set.is_none() => return ClassSlot::Method(method),
        _ => break,
      }
    }
    if get.is_none() && set.is_none() {
      ClassSlot::Missing
    } else {
      ClassSlot::Accessor { get, set }
    }
  }

  /// What `extends` of the class gives. Like oxc, the analysis assumes that the value is a
  /// constructor or `null`.
  fn class_parent(&mut self, class: &ClassValue<'a>) -> Parent<'a> {
    let node: &'a Class<'a> = class.node;
    let Some(heritage) = node.heritage_expression() else {
      return Parent::Builtin;
    };
    match self.value(heritage, class.site) {
      Value::Lit(Lit::Null) => Parent::Builtin,
      Value::Class(parent) => Parent::Ctor(Ctor::Class(parent)),
      Value::Function(func) if self.is_es5_ctor(&func) => Parent::Ctor(Ctor::Function(func)),
      _ => Parent::Unknown,
    }
  }

  /// The value of the static property `key` of the class.
  pub fn class_static_value(&mut self, class: ClassValue<'a>, key: &str) -> Value<'a> {
    if matches!(key, "__proto__" | "prototype")
      || self.is_class_escaped(&class)
      || self.is_entity_key_deopted(class.key(), key)
    {
      return Value::Unknown;
    }
    match self.class_slot(&class, key, true) {
      ClassSlot::Field(prop) => match &prop.value {
        Some(value) => self.init_value(value, class.site, class.holder),
        None => Value::Lit(Lit::Undefined),
      },
      ClassSlot::Method(method) => Value::Function(class_method_fn(&class, method)),
      // A getter of a class that did not escape does not read `this`, so it returns the same
      // value for each class.
      ClassSlot::Accessor { get: Some(getter), .. } => {
        self.return_value(class_method_fn(&class, getter))
      }
      ClassSlot::Accessor { get: None, .. } => Value::Lit(Lit::Undefined),
      ClassSlot::Missing => match self.class_parent(&class) {
        // Code can add a key to `Function.prototype` or `Object.prototype`. Like rollup, the
        // analysis assumes that the key is not an array index. See
        // internal-docs/advanced-tree-shaking/design.md.
        Parent::Builtin if array_index(key).is_none() => Value::Unknown,
        Parent::Builtin => Value::Lit(Lit::Undefined),
        Parent::Ctor(Ctor::Class(parent)) => {
          if self.state.depth >= MAX_DEPTH {
            return Value::Unknown;
          }
          self.state.depth += 1;
          let value = self.class_static_value(parent, key);
          self.state.depth -= 1;
          value
        }
        Parent::Ctor(Ctor::Function(_)) | Parent::Unknown => Value::Unknown,
      },
      ClassSlot::Unknown => Value::Unknown,
    }
  }

  /// Whether a read (`write: false`) or a write of the static property `key` of the class runs
  /// code or throws, apart from the write itself. With `defining`, the check runs inside the
  /// definition of the class, where only the class body decides.
  pub fn static_member_effects(
    &mut self,
    class: &ClassValue<'a>,
    key: &str,
    write: bool,
    defining: bool,
  ) -> bool {
    if !defining && self.is_class_escaped(class) {
      return true;
    }
    match key {
      // `__proto__` reads with a builtin getter, and a write changes the prototype.
      "__proto__" | "prototype" => return write,
      // The accessors of `Function.prototype` throw for a class.
      "caller" | "arguments" => return true,
      _ => {}
    }
    match self.class_slot(class, key, true) {
      ClassSlot::Field(_) | ClassSlot::Method(_) => false,
      ClassSlot::Accessor { get, set } => match if write { set } else { get } {
        Some(method) => {
          let func = class_method_fn(class, method);
          self.fn_uses_this(&func) || self.function_call_effects(func)
        }
        // A write to a getter-only property throws, and a read of a setter-only property gives
        // `undefined`.
        None => write,
      },
      // Non-writable own properties of the class.
      ClassSlot::Missing if matches!(key, "length" | "name") => write,
      ClassSlot::Missing => match self.class_parent(class) {
        Parent::Builtin => false,
        Parent::Ctor(Ctor::Class(parent)) => {
          if self.state.depth >= MAX_DEPTH {
            return true;
          }
          self.state.depth += 1;
          let effects = self.static_member_effects(&parent, key, write, false);
          self.state.depth -= 1;
          effects
        }
        // A function has data properties only, unless unknown code defined an accessor.
        Parent::Ctor(Ctor::Function(func)) => self.is_fn_object_escaped(&func),
        Parent::Unknown => true,
      },
      ClassSlot::Unknown => true,
    }
  }

  /// Whether a read or a write of `key` on an object whose prototype is the `prototype` of
  /// `ctor` runs code or throws, apart from the write itself. The own properties of such an
  /// object are data properties, or the object is the prototype itself, so the check starts at
  /// the members of the class.
  pub fn instance_member_effects(&mut self, ctor: Ctor<'a>, key: &str, write: bool) -> bool {
    if key == "__proto__" {
      return write;
    }
    if !self.is_proto_known(ctor) {
      return true;
    }
    let Ctor::Class(class) = ctor else {
      // The prototype of a function has data properties only.
      return false;
    };
    match self.class_slot(&class, key, false) {
      ClassSlot::Field(_) | ClassSlot::Method(_) => false,
      ClassSlot::Accessor { get, set } => match if write { set } else { get } {
        Some(method) => {
          let func = class_method_fn(&class, method);
          self.fn_uses_this(&func) || self.function_call_effects(func)
        }
        None => write,
      },
      ClassSlot::Missing => match self.class_parent(&class) {
        Parent::Builtin => false,
        Parent::Ctor(parent) => {
          if self.state.depth >= MAX_DEPTH {
            return true;
          }
          self.state.depth += 1;
          let effects = self.instance_member_effects(parent, key, write);
          self.state.depth -= 1;
          effects
        }
        Parent::Unknown => true,
      },
      ClassSlot::Unknown => true,
    }
  }

  /// The constructor whose `prototype` property `expr` reads.
  pub fn prototype_owner(&mut self, expr: &'a Expression<'a>, site: Site) -> Option<Ctor<'a>> {
    let member = expr.without_parentheses().as_member_expression()?;
    if self.member_key(member, site).as_deref() != Some("prototype") {
      return None;
    }
    match self.value(member.object(), site) {
      Value::Class(class) => Some(Ctor::Class(class)),
      Value::Function(func) if self.is_es5_ctor(&func) => Some(Ctor::Function(func)),
      _ => None,
    }
  }

  /// Whether the definition of the class runs code with effects: decorators, `extends`,
  /// computed keys, static fields, and static blocks. Like oxc, the analysis assumes that
  /// `extends` gets a constructor or `null`.
  pub fn class_effects(&mut self, class: &'a Class<'a>, cx: Cx<'a>) -> bool {
    if !class.decorators.is_empty() {
      return true;
    }
    if let Some(heritage) = class.heritage_expression()
      && self.expr_effects(heritage, cx)
    {
      return true;
    }
    for element in &class.body.body {
      let (decorators, key, computed, is_static) = match element {
        ClassElement::MethodDefinition(method) => {
          (&method.decorators, &method.key, method.computed, method.r#static)
        }
        ClassElement::PropertyDefinition(prop) => {
          (&prop.decorators, &prop.key, prop.computed, prop.r#static)
        }
        ClassElement::AccessorProperty(prop) => {
          (&prop.decorators, &prop.key, prop.computed, prop.r#static)
        }
        ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => continue,
      };
      if !decorators.is_empty() {
        return true;
      }
      if computed && let Some(key) = key.as_expression() {
        if self.expr_effects(key, cx) || !is_primitive_key(&self.value(key, cx.site)) {
          return true;
        }
        // The `prototype` of a class is not configurable, so defining it throws.
        if is_static && self.computed_key(key, cx.site).as_deref() == Some("prototype") {
          return true;
        }
      }
    }
    let static_cx =
      Cx { this: ThisValue::Class(ClassValue { site: cx.site, node: class, holder: None }), ..cx };
    for element in &class.body.body {
      let effects = match element {
        ClassElement::PropertyDefinition(prop) if prop.r#static => {
          prop.value.as_ref().is_some_and(|value| self.expr_effects(value, static_cx))
        }
        ClassElement::AccessorProperty(prop) if prop.r#static => {
          prop.value.as_ref().is_some_and(|value| self.expr_effects(value, static_cx))
        }
        ClassElement::StaticBlock(block) => self.stmts_effects(&block.body, static_cx),
        _ => false,
      };
      if effects {
        return true;
      }
    }
    false
  }

  /// The effects of `new` of `callee`, apart from the callee and the arguments. `None` when the
  /// analysis does not model the call. The new object is new, so writes to it have no effect
  /// unless the prototype chain runs code.
  pub fn construct_effects(
    &mut self,
    callee: &Value<'a>,
    new: &'a NewExpression<'a>,
    site: Site,
  ) -> Option<bool> {
    let ctor = match callee {
      Value::Function(func) if self.is_es5_ctor(func) => Ctor::Function(*func),
      Value::Class(class) => Ctor::Class(*class),
      _ => return None,
    };
    let this =
      if self.is_proto_known(ctor) { ThisValue::Instance(ctor) } else { ThisValue::Unknown };
    self.run_ctor_effects(ctor, this, new, site)
  }

  /// The function that gets the arguments of a `new` of `callee`: the function, or the
  /// constructor of the class. The default constructor of a derived class gives its arguments to
  /// the parent constructor.
  pub fn ctor_args_fn(&mut self, callee: &Value<'a>) -> Option<FnValue<'a>> {
    let mut class = match callee {
      Value::Function(func) => return Some(*func),
      Value::Class(class) => *class,
      _ => return None,
    };
    for _ in 0..MAX_DEPTH {
      let node: &'a Class<'a> = class.node;
      let constructor = node.body.body.iter().find_map(|element| match element {
        ClassElement::MethodDefinition(method)
          if method.kind == MethodDefinitionKind::Constructor =>
        {
          Some(method)
        }
        _ => None,
      });
      if let Some(method) = constructor {
        return Some(class_method_fn(&class, method));
      }
      node.heritage.as_ref()?;
      match self.class_parent(&class) {
        Parent::Ctor(Ctor::Class(parent)) => class = parent,
        Parent::Ctor(Ctor::Function(func)) => return Some(func),
        Parent::Builtin | Parent::Unknown => return None,
      }
    }
    None
  }

  /// The effects of the constructor `ctor` for `new`, where `this` is the new object. The default
  /// constructor of a derived class gives its arguments to the parent constructor, and then it
  /// defines the fields.
  fn run_ctor_effects(
    &mut self,
    ctor: Ctor<'a>,
    this: ThisValue<'a>,
    new: &'a NewExpression<'a>,
    site: Site,
  ) -> Option<bool> {
    let func = match ctor {
      Ctor::Function(func) => func,
      Ctor::Class(class) => {
        let node: &'a Class<'a> = class.node;
        let constructor = node.body.body.iter().find_map(|element| match element {
          ClassElement::MethodDefinition(method)
            if method.kind == MethodDefinitionKind::Constructor =>
          {
            Some(class_method_fn(&class, method))
          }
          _ => None,
        });
        if node.heritage.is_some() {
          // An explicit constructor of a derived class calls `super()`, which the analysis does
          // not model.
          if constructor.is_some() || !node.decorators.is_empty() {
            return None;
          }
          let parent_effects = match self.class_parent(&class) {
            // `super()` throws for `extends null`.
            Parent::Builtin => return Some(true),
            Parent::Ctor(parent) => {
              // The fields go on the object that the parent constructor gives, which can be an
              // object that rejects new properties.
              if collect::installs_instance_elements(node) && self.ctor_may_return_value(parent) {
                return None;
              }
              if self.state.depth >= MAX_DEPTH {
                return Some(true);
              }
              self.state.depth += 1;
              let effects = self.run_ctor_effects(parent, this, new, site);
              self.state.depth -= 1;
              effects?
            }
            Parent::Unknown => return None,
          };
          return if parent_effects { Some(true) } else { self.class_fields_effects(&class, this) };
        }
        if self.class_fields_effects(&class, this)? {
          return Some(true);
        }
        // The default constructor of a base class does nothing.
        let Some(func) = constructor else {
          return Some(false);
        };
        func
      }
    };
    self.record_args(&func, new.node_id(), CallArgs::Ast(&new.arguments), site);
    Some(self.invoke_effects(func, this, true))
  }

  /// Whether `new` of `ctor` can give an object other than the new object: a constructor in the
  /// chain returns a value.
  fn ctor_may_return_value(&mut self, ctor: Ctor<'a>) -> bool {
    let returns_value = |stmts: &'a [oxc::ast::ast::Statement<'a>]| {
      let mut returned = Vec::new();
      collect::return_arguments(stmts, &mut returned);
      !returned.is_empty()
    };
    match ctor {
      Ctor::Function(func) => returns_value(func.node.body_statements()),
      Ctor::Class(class) => {
        let node: &'a Class<'a> = class.node;
        let constructor = node.body.body.iter().find_map(|element| match element {
          ClassElement::MethodDefinition(method)
            if method.kind == MethodDefinitionKind::Constructor =>
          {
            Some(method)
          }
          _ => None,
        });
        if let Some(constructor) = constructor {
          return constructor
            .value
            .body
            .as_ref()
            .is_none_or(|body| returns_value(&body.statements));
        }
        if node.heritage.is_none() {
          return false;
        }
        match self.class_parent(&class) {
          Parent::Builtin => false,
          Parent::Ctor(parent) => {
            if self.state.depth >= MAX_DEPTH {
              return true;
            }
            self.state.depth += 1;
            let returns = self.ctor_may_return_value(parent);
            self.state.depth -= 1;
            returns
          }
          Parent::Unknown => true,
        }
      }
    }
  }

  /// The effects of the instance fields that `new` of the class defines. A field is a new own
  /// data property, so only its initializer can have effects. `None` when decorators can change
  /// a member.
  fn class_fields_effects(&mut self, class: &ClassValue<'a>, this: ThisValue<'a>) -> Option<bool> {
    let node: &'a Class<'a> = class.node;
    if !node.decorators.is_empty() {
      return None;
    }
    let cx = Cx { site: class.site.in_function(), callee_scope: None, this };
    for element in &node.body.body {
      match element {
        ClassElement::PropertyDefinition(prop) => {
          if !prop.decorators.is_empty() {
            return None;
          }
          if !prop.r#static && prop.value.as_ref().is_some_and(|value| self.expr_effects(value, cx))
          {
            return Some(true);
          }
        }
        ClassElement::MethodDefinition(method) if !method.decorators.is_empty() => return None,
        ClassElement::AccessorProperty(_) => return None,
        _ => {}
      }
    }
    Some(false)
  }
}
