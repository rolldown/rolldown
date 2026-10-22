use oxc::{
  ast::{
    AstKind,
    ast::{
      Argument, ArrayExpressionElement, AssignmentExpression, AssignmentOperator, AssignmentTarget,
      BinaryExpression, BinaryOperator, BindingPattern, CallExpression, ChainElement,
      ConditionalExpression, Declaration, ExportDefaultDeclarationKind, Expression, ForInStatement,
      ForOfStatement, ForStatementLeft, IdentifierReference, LogicalExpression, LogicalOperator,
      MemberExpression, NewExpression, ObjectExpression, ObjectProperty, ObjectPropertyKind,
      PropertyKind, SimpleAssignmentTarget, Statement, UnaryExpression, UnaryOperator,
      VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
    },
  },
  ast_visit::VisitJs,
  semantic::{NodeId, ScopeId, SymbolId},
};
use oxc_ecmascript::side_effects::MayHaveSideEffects;
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, SymbolRef};

use super::{
  Analyzer,
  classes::{ClassSlot, ThisValue},
  collect,
  env::{DeclKind, Init, OxcCtx, Site, is_manual_pure},
  facts::{CallRecord, LocalInit, LocalKey, NodeKey},
  value::{
    ArrValue, ClassValue, FnNode, FnValue, Lit, MAX_DEPTH, ObjValue, PatternPath, Proto, Resolved,
    Slot, Value, accessor_fn, array_index, has_proto_setter, is_primitive_key, is_proto_setter,
    pattern_path,
  },
  walk::EscapeVisitor,
};

/// The keys from a pattern path, for a deopt. An unknown path is one unknown key.
fn pattern_prefix(
  pattern: Option<&BindingPattern>,
  symbol_id: SymbolId,
) -> Vec<Option<CompactStr>> {
  match pattern.map(|pattern| pattern_path(pattern, symbol_id)) {
    None => Vec::new(),
    Some(PatternPath::Keys(keys)) => keys.into_iter().map(Some).collect(),
    Some(_) => vec![None],
  }
}

/// Applies the deopts of every write in code that the analysis does not check otherwise.
struct WriteDeopter<'w, 's, 'a> {
  an: &'w mut Analyzer<'s, 'a>,
  site: Site,
}

impl<'a> WriteDeopter<'_, '_, 'a> {
  fn target(&mut self, target: &'a AssignmentTarget<'a>) {
    match target {
      AssignmentTarget::AssignmentTargetIdentifier(ident) => self.ident(ident),
      target @ oxc::ast::match_member_expression!(AssignmentTarget) => {
        self.an.deopt_member_write(target.to_member_expression(), self.site);
      }
      target => {
        EscapeVisitor { an: &mut *self.an, module: self.site.module }
          .visit_assignment_target(target);
      }
    }
  }

  fn ident(&mut self, ident: &IdentifierReference<'a>) {
    match self.an.resolve(self.site.module, ident) {
      Resolved::Root(symbol_id) => {
        let canonical = self.an.env.canonical(self.site.module, symbol_id);
        self.an.state.mark_reassigned(canonical);
      }
      Resolved::Local(symbol_id) => {
        self.an.state.mark_reassigned(SymbolRef::from((self.site.module, symbol_id)));
      }
      Resolved::Global | Resolved::None => {}
    }
  }
}

impl<'a> VisitJs<'a> for WriteDeopter<'_, '_, 'a> {
  fn enter_node(&mut self, kind: AstKind<'a>) {
    match kind {
      AstKind::AssignmentExpression(assign) => self.target(&assign.left),
      AstKind::UpdateExpression(update) => match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(ident) => self.ident(ident),
        target @ oxc::ast::match_member_expression!(SimpleAssignmentTarget) => {
          self.an.deopt_member_write(target.to_member_expression(), self.site);
        }
        target => EscapeVisitor { an: &mut *self.an, module: self.site.module }
          .visit_simple_assignment_target(target),
      },
      AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Delete => {
        if let Some(member) = unary.argument.without_parentheses().as_member_expression() {
          self.an.deopt_member_delete(member, self.site);
        }
      }
      AstKind::ForInStatement(ForInStatement { left, .. })
      | AstKind::ForOfStatement(ForOfStatement { left, .. }) => {
        if let Some(target) = left.as_assignment_target() {
          self.target(target);
        }
      }
      _ => {}
    }
  }
}

/// The context of an effect check.
#[derive(Debug, Clone, Copy)]
pub struct Cx<'a> {
  pub site: Site,
  /// The scope of the function whose call the check analyzes. A write to a `var` or a
  /// parameter of that function is not visible after the call. `None` when the check decides
  /// whether code can go, and then every write is visible.
  pub callee_scope: Option<ScopeId>,
  /// The value of `this` in the code that the check analyzes.
  pub this: ThisValue<'a>,
}

impl Cx<'_> {
  pub fn removal(site: Site) -> Self {
    Self { site, callee_scope: None, this: ThisValue::Unknown }
  }
}

/// The effect checker. A result of `false` means that removing the code changes nothing, so
/// the check must visit every write in the code and apply its deopts: the removal decision can
/// depend on facts that the write changes. A result of `true` keeps the code, and then the walker
/// visits it, so the check can stop early.
impl<'a> Analyzer<'_, 'a> {
  /// Apply the deopt of each write in `stmt`, which the effect checks do not visit.
  pub fn deopt_writes(&mut self, stmt: &'a Statement<'a>, site: Site) {
    WriteDeopter { an: self, site }.visit_statement(stmt);
  }

  pub fn stmts_effects(&mut self, stmts: &'a [Statement<'a>], cx: Cx<'a>) -> bool {
    self.register_local_fns(stmts, cx.site);
    for stmt in stmts {
      if self.stmt_effects(stmt, cx) {
        return true;
      }
      // The statements after a jump never run.
      if self.always_jumps(stmt, cx.site) {
        break;
      }
    }
    false
  }

  /// Whether `stmt` ends with a jump on each path that the known values allow.
  fn always_jumps(&mut self, stmt: &'a Statement<'a>, site: Site) -> bool {
    match stmt {
      Statement::ReturnStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::BreakStatement(_)
      | Statement::ContinueStatement(_) => true,
      Statement::BlockStatement(block) => {
        block.body.iter().any(|stmt| self.always_jumps(stmt, site))
      }
      Statement::IfStatement(if_stmt) => match self.value(&if_stmt.test, site).truthy() {
        Some(true) => self.always_jumps(&if_stmt.consequent, site),
        Some(false) => {
          if_stmt.alternate.as_ref().is_some_and(|alternate| self.always_jumps(alternate, site))
        }
        None => {
          self.always_jumps(&if_stmt.consequent, site)
            && if_stmt
              .alternate
              .as_ref()
              .is_some_and(|alternate| self.always_jumps(alternate, site))
        }
      },
      _ => false,
    }
  }

  /// A `for-in` loop over a known object enumerates its keys, which runs no user code.
  fn for_in_effects(&mut self, for_in: &'a ForInStatement<'a>, cx: Cx<'a>) -> bool {
    let fresh_binding = matches!(
      &for_in.left,
      ForStatementLeft::VariableDeclaration(decl)
        if matches!(decl.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Const)
          && decl.declarations.len() == 1
          && matches!(decl.declarations[0].id, BindingPattern::BindingIdentifier(_))
    );
    if !fresh_binding || self.expr_effects(&for_in.right, cx) {
      return true;
    }
    let enumerable = match self.value(&for_in.right, cx.site) {
      // A proxy in the prototype chain can trap the enumeration.
      Value::Object(obj) => !self.is_object_escaped(&obj) && !has_proto_setter(obj.node),
      Value::Array(arr) => !self.is_array_escaped(&arr),
      Value::Lit(_) => true,
      _ => false,
    };
    !enumerable || self.stmt_effects(&for_in.body, cx)
  }

  pub fn stmt_effects(&mut self, stmt: &'a Statement<'a>, cx: Cx<'a>) -> bool {
    match stmt {
      Statement::ExpressionStatement(stmt) => self.expr_effects(&stmt.expression, cx),
      // A local export list evaluates nothing.
      Statement::EmptyStatement(_)
      | Statement::FunctionDeclaration(_)
      | Statement::ExportNamedDeclaration(_) => false,
      Statement::BlockStatement(block) => self.stmts_effects(&block.body, cx),
      Statement::IfStatement(if_stmt) => {
        if self.expr_effects(&if_stmt.test, cx) {
          return true;
        }
        let test = self.value(&if_stmt.test, cx.site).truthy();
        if test != Some(false) && self.stmt_effects(&if_stmt.consequent, cx) {
          return true;
        }
        test != Some(true)
          && if_stmt.alternate.as_ref().is_some_and(|alternate| self.stmt_effects(alternate, cx))
      }
      Statement::ReturnStatement(ret) => {
        cx.callee_scope.is_none()
          || ret.argument.as_ref().is_some_and(|arg| self.expr_effects(arg, cx))
      }
      Statement::VariableDeclaration(decl) => self.var_decl_effects(decl, cx),
      Statement::ClassDeclaration(class) => self.class_effects(class, cx),
      Statement::ForInStatement(for_in) => self.for_in_effects(for_in, cx),
      Statement::TryStatement(try_stmt) => {
        self.stmts_effects(&try_stmt.block.body, cx)
          || try_stmt
            .finalizer
            .as_ref()
            .is_some_and(|finalizer| self.stmts_effects(&finalizer.body, cx))
      }
      Statement::ExportDeclaration(export) => self.decl_effects(&export.declaration, cx),
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(_)
        | ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => false,
        ExportDefaultDeclarationKind::ClassDeclaration(class) => self.class_effects(class, cx),
        expr => self.expr_effects(expr.to_expression(), cx),
      },
      _ => true,
    }
  }

  fn decl_effects(&mut self, decl: &'a Declaration<'a>, cx: Cx<'a>) -> bool {
    match decl {
      Declaration::VariableDeclaration(decl) => self.var_decl_effects(decl, cx),
      Declaration::FunctionDeclaration(_)
      | Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_) => false,
      Declaration::ClassDeclaration(class) => self.class_effects(class, cx),
      _ => true,
    }
  }

  fn var_decl_effects(&mut self, decl: &'a VariableDeclaration<'a>, cx: Cx<'a>) -> bool {
    if matches!(decl.kind, VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing) {
      return true;
    }
    for declarator in &decl.declarations {
      self.register_local_decl(declarator, cx.site);
      let Some(init) = &declarator.init else {
        continue;
      };
      if self.expr_effects(init, cx) {
        return true;
      }
      if !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
        let value = self.value(init, cx.site);
        if self.pattern_effects(&declarator.id, value, cx) {
          return true;
        }
      }
    }
    false
  }

  /// Record the initializer of each binding that a declaration inside a function declares, so
  /// that a write deopt can follow it.
  pub fn register_local_decl(&mut self, declarator: &'a VariableDeclarator<'a>, site: Site) {
    let Some(init) = &declarator.init else {
      return;
    };
    let pattern = match &declarator.id {
      BindingPattern::BindingIdentifier(_) => None,
      pattern => Some(pattern),
    };
    for id in declarator.id.get_binding_identifiers() {
      if let Some(symbol_id) = id.symbol_id.get()
        && !self.env.symbols.is_declared_in_root_scope((site.module, symbol_id).into())
      {
        self.state.register_local_init((site.module, symbol_id), LocalInit { init, pattern, site });
      }
    }
  }

  /// Whether destructuring `value` with `pattern` can run code or throw.
  pub fn pattern_effects(
    &mut self,
    pattern: &'a BindingPattern<'a>,
    value: Value<'a>,
    cx: Cx<'a>,
  ) -> bool {
    match pattern {
      BindingPattern::BindingIdentifier(_) => false,
      // Array destructuring runs the iterator protocol.
      BindingPattern::ArrayPattern(_) => true,
      BindingPattern::AssignmentPattern(assign) => {
        // The default runs when the value is undefined.
        let may_be_undefined = match &value {
          Value::Lit(lit) => matches!(lit, Lit::Undefined),
          Value::Object(_)
          | Value::Array(_)
          | Value::Function(_)
          | Value::Class(_)
          | Value::Namespace(_)
          | Value::Symbol => false,
          Value::Unknown => true,
        };
        if may_be_undefined && self.expr_effects(&assign.right, cx) {
          return true;
        }
        let value = match value {
          Value::Lit(Lit::Undefined) => self.value(&assign.right, cx.site),
          Value::Unknown => Value::Unknown,
          value => value,
        };
        self.pattern_effects(&assign.left, value, cx)
      }
      BindingPattern::ObjectPattern(obj) => {
        if self.env.property_reads_always {
          return true;
        }
        let known = match &value {
          Value::Object(object) => !self.is_object_escaped(object),
          // Destructuring `null` or `undefined` throws.
          Value::Lit(Lit::Undefined | Lit::Null) => return true,
          _ => false,
        };
        if !known && self.env.flat_options.property_read_side_effects() {
          // A getter can run, and a `null` or `undefined` value throws.
          return true;
        }
        for prop in &obj.properties {
          let key = self.binding_key(prop, cx.site);
          if prop.computed
            && let Some(key_expr) = prop.key.as_expression()
            && (self.expr_effects(key_expr, cx)
              || (key.is_none() && !is_primitive_key(&self.value(key_expr, cx.site))))
          {
            return true;
          }
          let sub = match &value {
            Value::Object(object) if known => {
              if self.object_read_effects(object, key.as_deref()) {
                return true;
              }
              self.member_value(value.clone(), key.as_deref(), cx.site)
            }
            _ => Value::Unknown,
          };
          if self.pattern_effects(&prop.value, sub, cx) {
            return true;
          }
        }
        match (&obj.rest, &value) {
          (Some(_), Value::Object(object)) if known => self.object_getter_at(object, None),
          _ => false,
        }
      }
    }
  }

  pub fn expr_effects(&mut self, expr: &'a Expression<'a>, cx: Cx<'a>) -> bool {
    if self.state.depth >= MAX_DEPTH {
      return true;
    }
    self.state.depth += 1;
    let effects = self.expr_effects_inner(expr, cx);
    self.state.depth -= 1;
    effects
  }

  fn expr_effects_inner(&mut self, expr: &'a Expression<'a>, cx: Cx<'a>) -> bool {
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::StringLiteral(_)
      | Expression::ThisExpression(_)
      | Expression::ImportMeta(_)
      | Expression::NewTarget(_)
      | Expression::FunctionExpression(_)
      | Expression::ArrowFunctionExpression(_) => false,
      Expression::Identifier(ident) => self.ident_read_effects(ident, cx),
      Expression::ParenthesizedExpression(paren) => self.expr_effects(&paren.expression, cx),
      Expression::SequenceExpression(seq) => {
        seq.expressions.iter().any(|expr| self.expr_effects(expr, cx))
      }
      Expression::TemplateLiteral(template) => {
        let mut all_primitive = true;
        for expr in &template.expressions {
          if self.expr_effects(expr, cx) {
            return true;
          }
          all_primitive &= matches!(self.value(expr, cx.site), Value::Lit(_));
        }
        // Converting an object to a string can run user code.
        !all_primitive && self.oxc_effects(expr, cx)
      }
      Expression::ObjectExpression(obj) => self.object_literal_effects(obj, cx),
      Expression::ArrayExpression(arr) => {
        for element in &arr.elements {
          let effects = match element {
            ArrayExpressionElement::SpreadElement(spread) => {
              self.expr_effects(&spread.argument, cx)
                || self.iterable_spread_effects(&spread.argument, cx)
            }
            ArrayExpressionElement::Elision(_) => false,
            _ => self.expr_effects(element.to_expression(), cx),
          };
          if effects {
            return true;
          }
        }
        false
      }
      Expression::UnaryExpression(unary) => self.unary_effects(unary, expr, cx),
      Expression::BinaryExpression(binary) => self.binary_effects(binary, expr, cx),
      Expression::LogicalExpression(logical) => self.logical_effects(logical, cx),
      Expression::ConditionalExpression(cond) => self.conditional_effects(cond, cx),
      Expression::AssignmentExpression(assign) => self.assignment_effects(assign, cx),
      Expression::UpdateExpression(update) => {
        self.update_deopts(&update.argument, cx);
        true
      }
      Expression::CallExpression(call) => self.call_effects(call, cx),
      Expression::NewExpression(new) => self.new_effects(new, expr, cx),
      Expression::ChainExpression(chain) => match &chain.expression {
        ChainElement::CallExpression(call) => self.call_effects(call, cx),
        ChainElement::TSNonNullExpression(expr) => self.expr_effects(&expr.expression, cx),
        element => self.member_read_effects(element.to_member_expression(), cx),
      },
      oxc::ast::match_member_expression!(Expression) => {
        self.member_read_effects(expr.to_member_expression(), cx)
      }
      Expression::ClassExpression(class) => self.class_effects(class, cx),
      Expression::TaggedTemplateExpression(_)
      | Expression::PrivateInExpression(_)
      | Expression::JSXElement(_)
      | Expression::JSXFragment(_) => self.oxc_effects(expr, cx),
      _ => true,
    }
  }

  fn ident_read_effects(&self, ident: &IdentifierReference<'a>, cx: Cx<'a>) -> bool {
    match self.resolve(cx.site.module, ident) {
      Resolved::Global => self
        .env
        .module(cx.site.module)
        .is_none_or(|facts| ident.may_have_side_effects(&OxcCtx { env: self.env, facts })),
      Resolved::Root(symbol_id) => {
        let canonical = self.env.canonical(cx.site.module, symbol_id);
        self.tdz_effects(canonical, cx.site)
      }
      Resolved::Local(_) | Resolved::None => false,
    }
  }

  /// Whether a read of `binding` at `site` can see it uninitialized and throw.
  fn tdz_effects(&self, binding: SymbolRef, site: Site) -> bool {
    match self.env.decl(binding) {
      Some(decl) => {
        matches!(decl.kind, DeclKind::Let | DeclKind::Const | DeclKind::Class | DeclKind::Default)
          && !self.env.is_init_safe(site, binding, &decl)
      }
      // A binding of a module that the analysis does not cover. An analyzed module has no
      // declaration only for bindings that never throw, such as its namespace object.
      None => {
        self.env.module(binding.owner).is_none()
          && !self.env.is_foreign_init_safe(site, binding.owner)
      }
    }
  }

  fn member_read_effects(&mut self, member: &'a MemberExpression<'a>, cx: Cx<'a>) -> bool {
    if self.expr_effects(member.object(), cx) {
      return true;
    }
    let key = match member {
      MemberExpression::StaticMemberExpression(member) => {
        Some(CompactStr::new(member.property.name.as_str()))
      }
      MemberExpression::ComputedMemberExpression(computed) => {
        if self.expr_effects(&computed.expression, cx) {
          return true;
        }
        let key = self.computed_key(&computed.expression, cx.site);
        if key.is_none() && !is_primitive_key(&self.value(&computed.expression, cx.site)) {
          // Converting an object to a property key can run user code.
          return true;
        }
        key
      }
      MemberExpression::PrivateFieldExpression(_) => return true,
    };
    let object = self.value(member.object(), cx.site);
    if self.env.property_reads_always && !matches!(object, Value::Namespace(_)) {
      return true;
    }
    // A new object with builtin properties only.
    if let Expression::RegExpLiteral(_) = member.object().without_parentheses() {
      return false;
    }
    if let Expression::ThisExpression(_) = member.object().without_parentheses()
      && let Some(key) = key.as_deref()
    {
      match cx.this {
        ThisValue::Instance(ctor) => return self.instance_member_effects(ctor, key, false),
        ThisValue::Class(class) => return self.static_member_effects(&class, key, false, true),
        ThisValue::Unknown => {}
      }
    }
    match object {
      Value::Lit(Lit::Undefined | Lit::Null) => !member.optional(),
      Value::Lit(_) | Value::Symbol => false,
      Value::Object(obj) => {
        if self.is_object_escaped(&obj) {
          self.oxc_member_effects(member, cx)
        } else {
          self.object_read_effects(&obj, key.as_deref())
        }
      }
      Value::Namespace(module) => match key.as_deref() {
        Some(key) => self
          .env
          .namespace_export(module, key)
          .is_some_and(|export| self.tdz_effects(export, cx.site)),
        None => true,
      },
      // The elements are data properties, and the prototype chain has builtins only.
      Value::Array(arr) => self.is_array_escaped(&arr) && self.oxc_member_effects(member, cx),
      // The own properties of a function are data properties, and so are the properties of
      // `Function.prototype`, apart from `caller` and `arguments`, which throw. Only unknown code
      // can define an accessor on the function.
      Value::Function(func) => match key.as_deref() {
        Some(key)
          if !matches!(key, "caller" | "arguments") && !self.is_fn_object_escaped(&func) =>
        {
          false
        }
        _ => self.oxc_member_effects(member, cx),
      },
      Value::Class(class) => match key.as_deref() {
        Some(key) if !self.is_class_escaped(&class) => {
          self.static_member_effects(&class, key, false, false)
        }
        _ => self.oxc_member_effects(member, cx),
      },
      Value::Unknown => match (key.as_deref(), self.prototype_owner(member.object(), cx.site)) {
        (Some(key), Some(ctor)) if self.is_proto_known(ctor) => {
          self.instance_member_effects(ctor, key, false)
        }
        _ => self.oxc_member_effects(member, cx),
      },
    }
  }

  /// Whether a read of `key` (`None`: an unknown key) of an object literal that did not escape
  /// can run code. A getter runs, and the analysis checks its call.
  fn object_read_effects(&mut self, obj: &ObjValue<'a>, key: Option<&str>) -> bool {
    let Some(key) = key else {
      return self.object_getter_at(obj, None) || has_proto_setter(obj.node);
    };
    if self.is_key_deleted(obj, key) {
      return self.object_getter_at(obj, Some(key)) || self.proto_read_effects(obj, key);
    }
    match self.own_slot(obj, key) {
      Slot::Data(_) | Slot::Accessor { get: None, .. } => false,
      Slot::Accessor { get: Some(getter), .. } => {
        accessor_fn(obj, getter).is_none_or(|func| self.function_call_effects(func))
      }
      Slot::Missing => self.proto_read_effects(obj, key),
      Slot::Unknown => self.object_getter_at(obj, Some(key)) || self.proto_read_effects(obj, key),
    }
  }

  /// Whether a read of `key` that reaches the prototype of the literal can run code.
  fn proto_read_effects(&mut self, obj: &ObjValue<'a>, key: &str) -> bool {
    match self.proto_of(obj) {
      Proto::Default | Proto::Null => false,
      Proto::Object(proto) => {
        if self.state.depth >= MAX_DEPTH || self.is_object_escaped(&proto) {
          return true;
        }
        self.state.depth += 1;
        let effects = self.object_read_effects(&proto, Some(key));
        self.state.depth -= 1;
        effects
      }
      Proto::Unknown => true,
    }
  }

  /// Whether a write of `key` to an object literal that did not escape runs code or throws,
  /// apart from the write itself: a setter runs, and a write to a getter-only property throws.
  fn object_write_effects(&mut self, obj: &ObjValue<'a>, key: &str) -> bool {
    if self.is_key_deleted(obj, key) {
      return self.object_accessor_at(obj, Some(key), |kind| kind != PropertyKind::Init)
        || self.proto_write_effects(obj, key);
    }
    match self.own_slot(obj, key) {
      Slot::Data(_) => false,
      Slot::Accessor { set: Some(setter), .. } => {
        accessor_fn(obj, setter).is_none_or(|func| self.function_call_effects(func))
      }
      Slot::Accessor { set: None, .. } => true,
      Slot::Missing => self.proto_write_effects(obj, key),
      Slot::Unknown => {
        self.object_accessor_at(obj, Some(key), |kind| kind != PropertyKind::Init)
          || self.proto_write_effects(obj, key)
      }
    }
  }

  /// Whether a write of `key` that the prototype of the literal handles runs code or throws.
  fn proto_write_effects(&mut self, obj: &ObjValue<'a>, key: &str) -> bool {
    match self.proto_of(obj) {
      Proto::Default | Proto::Null => false,
      Proto::Object(proto) => {
        if self.state.depth >= MAX_DEPTH || self.is_object_escaped(&proto) {
          return true;
        }
        self.state.depth += 1;
        let effects = self.object_write_effects(&proto, key);
        self.state.depth -= 1;
        effects
      }
      Proto::Unknown => true,
    }
  }

  /// Whether the object literal has a getter that a read of `key` can run. `None` is an
  /// unknown key.
  fn object_getter_at(&mut self, obj: &ObjValue<'a>, key: Option<&str>) -> bool {
    self.object_accessor_at(obj, key, |kind| kind == PropertyKind::Get)
  }

  fn object_accessor_at(
    &mut self,
    obj: &ObjValue<'a>,
    key: Option<&str>,
    matches_kind: impl Fn(PropertyKind) -> bool,
  ) -> bool {
    let site = obj.site;
    for prop in &obj.node.properties {
      let ObjectPropertyKind::ObjectProperty(prop) = prop else {
        continue;
      };
      if !matches_kind(prop.kind) {
        continue;
      }
      let Some(key) = key else {
        return true;
      };
      match self.property_key(prop, site) {
        Some(name) if name != key => {}
        _ => return true,
      }
    }
    false
  }

  fn call_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx<'a>) -> bool {
    if let Some(member) = call.callee.without_parentheses().as_member_expression() {
      if self.member_read_effects(member, cx) {
        return true;
      }
    } else if self.expr_effects(&call.callee, cx) {
      return true;
    }
    if self.arguments_effects(&call.arguments, cx) {
      return true;
    }
    if (call.pure && self.env.annotations()) || is_manual_pure(self.env, &call.callee) {
      return false;
    }
    if let Some(effects) = self.builtin_call_effects(call, cx) {
      return effects;
    }
    match self.value(&call.callee, cx.site) {
      Value::Function(func) => {
        self.record_call(&func, call, cx.site);
        self.function_call_effects(func)
      }
      Value::Unknown => self
        .env
        .module(cx.site.module)
        .is_none_or(|facts| call.may_have_side_effects(&OxcCtx { env: self.env, facts })),
      Value::Lit(Lit::Undefined | Lit::Null) => !call.optional,
      // Calling a value that is not a function throws.
      _ => true,
    }
  }

  fn arguments_effects(&mut self, arguments: &'a [Argument<'a>], cx: Cx<'a>) -> bool {
    for arg in arguments {
      let effects = match arg {
        Argument::SpreadElement(spread) => {
          self.expr_effects(&spread.argument, cx)
            || self.iterable_spread_effects(&spread.argument, cx)
        }
        _ => self.expr_effects(arg.to_expression(), cx),
      };
      if effects {
        return true;
      }
    }
    false
  }

  fn new_effects(
    &mut self,
    new: &'a NewExpression<'a>,
    expr: &'a Expression<'a>,
    cx: Cx<'a>,
  ) -> bool {
    if self.expr_effects(&new.callee, cx) || self.arguments_effects(&new.arguments, cx) {
      return true;
    }
    if new.pure && self.env.annotations() {
      return false;
    }
    let callee = self.value(&new.callee, cx.site);
    self.construct_effects(&callee, new, cx.site).unwrap_or_else(|| self.oxc_effects(expr, cx))
  }

  /// Whether a call of `func` has effects outside the call. A recursive call assumes that it
  /// has none, like rollup does.
  pub fn function_call_effects(&mut self, func: FnValue<'a>) -> bool {
    self.invoke_effects(func, ThisValue::Unknown, false)
  }

  /// Whether a call of `func`, or `new` of it (`construct`), has effects outside the call. `this`
  /// is the value of `this` in the body. The results of calls and of constructions have separate
  /// caches, because the value of `this` differs.
  pub fn invoke_effects(
    &mut self,
    func: FnValue<'a>,
    this: ThisValue<'a>,
    construct: bool,
  ) -> bool {
    let key = func.key();
    let cache = if construct { &self.state.construct_effects } else { &self.state.call_effects };
    if let Some(effects) = cache.get(&key) {
      return *effects;
    }
    let active = if construct { &self.state.active_constructs } else { &self.state.active_calls };
    if active.contains(&key) {
      self.state.used_assumption = true;
      return false;
    }
    if self.state.depth >= MAX_DEPTH {
      return true;
    }
    let active =
      if construct { &mut self.state.active_constructs } else { &mut self.state.active_calls };
    active.insert(key);
    self.register_params(&func);
    let saved_assumption = std::mem::replace(&mut self.state.used_assumption, false);
    let cx = Cx { site: func.body_site(), callee_scope: func.node.scope_id(), this };
    // A generator call runs only the parameter defaults.
    let effects = self.params_effects(&func, cx)
      || (!func.node.is_generator()
        && match func.node.expression_body() {
          Some(expr) => self.expr_effects(expr, cx),
          None => self.stmts_effects(func.node.body_statements(), cx),
        });
    let active =
      if construct { &mut self.state.active_constructs } else { &mut self.state.active_calls };
    active.remove(&key);
    let used_assumption = std::mem::replace(&mut self.state.used_assumption, saved_assumption);
    if used_assumption {
      self.state.used_assumption = true;
    } else if construct {
      self.state.construct_effects.insert(key, effects);
    } else {
      self.state.call_effects.insert(key, effects);
    }
    effects
  }

  fn params_effects(&mut self, func: &FnValue<'a>, cx: Cx<'a>) -> bool {
    let params = func.node.params();
    for (index, item) in params.items.iter().enumerate() {
      if item.initializer.as_ref().is_some_and(|init| self.expr_effects(init, cx)) {
        return true;
      }
      if !matches!(item.pattern, BindingPattern::BindingIdentifier(_)) {
        let value = if item.initializer.is_some() {
          Value::Unknown
        } else {
          self.arg_join(func.module, func.node, index)
        };
        if self.pattern_effects(&item.pattern, value, cx) {
          return true;
        }
      }
    }
    params
      .rest
      .as_ref()
      .is_some_and(|rest| !matches!(rest.rest.argument, BindingPattern::BindingIdentifier(_)))
  }

  /// Record a call of a known function. The write deopts of the parameters apply to the new
  /// arguments.
  pub fn record_call(&mut self, func: &FnValue<'a>, call: &'a CallExpression<'a>, site: Site) {
    self.record_args(func, call.node_id(), &call.arguments, site);
  }

  /// Record a call or a `new` (the node `node`) of a known function, with its arguments.
  pub fn record_args(
    &mut self,
    func: &FnValue<'a>,
    node: NodeId,
    args: &'a [Argument<'a>],
    site: Site,
  ) {
    self.register_params(func);
    let record = CallRecord { site, args };
    if !self.state.record_call(func.key(), (site.module, node), record) {
      return;
    }
    let deopts = self.state.arg_deopts.get(&func.key()).cloned().unwrap_or_default();
    for (index, path) in deopts {
      self.deopt_call_arg(&record, index, &path, 0);
    }
  }

  fn assignment_effects(&mut self, assign: &'a AssignmentExpression<'a>, cx: Cx<'a>) -> bool {
    let compound = assign.operator != AssignmentOperator::Assign;
    let target_effects = match &assign.left {
      AssignmentTarget::AssignmentTargetIdentifier(ident) => {
        self.ident_write_effects(ident, compound, cx)
      }
      target @ oxc::ast::match_member_expression!(AssignmentTarget) => {
        self.member_write_effects(target.to_member_expression(), compound, cx)
      }
      target => {
        EscapeVisitor { an: self, module: cx.site.module }.visit_assignment_target(target);
        true
      }
    };
    target_effects || self.expr_effects(&assign.right, cx)
  }

  fn update_deopts(&mut self, target: &'a SimpleAssignmentTarget<'a>, cx: Cx<'a>) {
    match target {
      SimpleAssignmentTarget::AssignmentTargetIdentifier(ident) => {
        self.ident_write_effects(ident, true, cx);
      }
      target @ oxc::ast::match_member_expression!(SimpleAssignmentTarget) => {
        self.member_write_effects(target.to_member_expression(), true, cx);
      }
      target => {
        EscapeVisitor { an: self, module: cx.site.module }.visit_simple_assignment_target(target);
      }
    }
  }

  /// A write to a binding that nothing includes has no effect, like rollup's
  /// `LocalVariable.hasEffectsOnInteractionAtPath`. The write still makes the value unknown.
  fn ident_write_effects(
    &mut self,
    ident: &IdentifierReference<'a>,
    compound: bool,
    cx: Cx<'a>,
  ) -> bool {
    match self.resolve(cx.site.module, ident) {
      Resolved::Root(symbol_id) => {
        let canonical = self.env.canonical(cx.site.module, symbol_id);
        self.state.mark_reassigned(canonical);
        if compound
          || self.env.module(cx.site.module).is_none_or(|facts| facts.is_named_import(symbol_id))
        {
          return true;
        }
        let Some(decl) = self.env.decl(canonical) else {
          return true;
        };
        match decl.kind {
          DeclKind::Var | DeclKind::Function => self.state.is_included(canonical),
          DeclKind::Let | DeclKind::Class => {
            self.state.is_included(canonical) || !self.env.is_init_safe(cx.site, canonical, &decl)
          }
          DeclKind::Const | DeclKind::Default | DeclKind::Other => true,
        }
      }
      Resolved::Local(symbol_id) => {
        self.state.mark_reassigned(SymbolRef::from((cx.site.module, symbol_id)));
        compound || !self.is_callee_var(cx, symbol_id)
      }
      Resolved::Global | Resolved::None => true,
    }
  }

  /// Whether `symbol_id` is a `var` or a parameter of the function whose call `cx` analyzes.
  fn is_callee_var(&self, cx: Cx<'a>, symbol_id: SymbolId) -> bool {
    let (Some(callee_scope), Some(facts)) = (cx.callee_scope, self.env.module(cx.site.module))
    else {
      return false;
    };
    let scoping = facts.scopes.scoping();
    if !scoping.symbol_flags(symbol_id).is_function_scoped_declaration() {
      return false;
    }
    let mut scope = Some(scoping.symbol_scope_id(symbol_id));
    while let Some(scope_id) = scope {
      if scope_id == callee_scope {
        return true;
      }
      scope = scoping.scope_parent_id(scope_id);
    }
    false
  }

  /// A property write has no effect when nothing includes the object and the property has no
  /// accessor. The write still makes the property unknown.
  fn member_write_effects(
    &mut self,
    member: &'a MemberExpression<'a>,
    compound: bool,
    cx: Cx<'a>,
  ) -> bool {
    self.deopt_member_write(member, cx.site);
    let key = self.member_key(member, cx.site);
    let object = self.value(member.object(), cx.site);
    if self.expr_effects(member.object(), cx) {
      return true;
    }
    match member {
      MemberExpression::ComputedMemberExpression(computed) => {
        if self.expr_effects(&computed.expression, cx)
          || (key.is_none() && !is_primitive_key(&self.value(&computed.expression, cx.site)))
        {
          return true;
        }
      }
      MemberExpression::PrivateFieldExpression(_) => return true,
      MemberExpression::StaticMemberExpression(_) => {}
    }
    if compound
      || self
        .member_root(member.object(), cx.site.module)
        .is_some_and(|root| self.state.is_included(root))
    {
      return true;
    }
    let Some(key) = key else {
      return true;
    };
    if let Expression::ThisExpression(_) = member.object().without_parentheses() {
      match cx.this {
        ThisValue::Instance(ctor) => return self.instance_member_effects(ctor, &key, true),
        ThisValue::Class(class) => return self.static_member_effects(&class, &key, true, true),
        ThisValue::Unknown => {}
      }
    }
    match object {
      Value::Object(obj) => {
        key == "__proto__"
          || self.is_object_escaped(&obj)
          || self.is_frozen(&obj)
          || self.object_write_effects(&obj, &key)
          || match obj.holder {
            Some(holder) => self.state.is_included(holder),
            // Included code reads the objects of a literal without a holder only at the paths of
            // the walks of the literal.
            None => {
              self.state.entity(obj.key()).is_some_and(|entity| entity.demand.at(&key).is_some())
            }
          }
      }
      Value::Array(arr) => key == "__proto__" || self.array_write_effects(&arr),
      Value::Function(func) => {
        // A new prototype can bring setters.
        matches!(
          key.as_str(),
          "name" | "length" | "prototype" | "caller" | "arguments" | "__proto__"
        ) || self.is_fn_object_escaped(&func)
          || func.holder.is_none_or(|holder| self.state.is_included(holder))
      }
      Value::Class(class) => {
        self.static_member_effects(&class, &key, true, false)
          || class.holder.is_none_or(|holder| self.state.is_included(holder))
      }
      // A write to the prototype of a constructor.
      Value::Unknown => match self.prototype_owner(member.object(), cx.site) {
        Some(ctor) => {
          self.instance_member_effects(ctor, &key, true)
            || ctor.holder().is_none_or(|holder| self.state.is_included(holder))
        }
        None => true,
      },
      _ => true,
    }
  }

  /// A `delete` of a property of an object literal that nothing includes has no effect. The
  /// properties of a literal can always go, so the `delete` cannot throw. It still makes the
  /// property unknown.
  fn delete_effects(&mut self, member: &'a MemberExpression<'a>, cx: Cx<'a>) -> bool {
    self.deopt_member_delete(member, cx.site);
    if self.expr_effects(member.object(), cx) {
      return true;
    }
    let key = match member {
      MemberExpression::StaticMemberExpression(member) => {
        Some(CompactStr::new(member.property.name.as_str()))
      }
      MemberExpression::ComputedMemberExpression(computed) => {
        if self.expr_effects(&computed.expression, cx)
          || !is_primitive_key(&self.value(&computed.expression, cx.site))
        {
          return true;
        }
        self.computed_key(&computed.expression, cx.site)
      }
      MemberExpression::PrivateFieldExpression(_) => return true,
    };
    let Some(key) = key else {
      return true;
    };
    if self
      .member_root(member.object(), cx.site.module)
      .is_some_and(|root| self.state.is_included(root))
    {
      return true;
    }
    match self.value(member.object(), cx.site) {
      Value::Object(obj) => {
        key == "__proto__"
          || self.is_object_escaped(&obj)
          || self.is_frozen(&obj)
          || match obj.holder {
            Some(holder) => self.state.is_included(holder),
            None => {
              self.state.entity(obj.key()).is_some_and(|entity| entity.demand.at(&key).is_some())
            }
          }
      }
      _ => true,
    }
  }

  fn is_frozen(&self, obj: &ObjValue<'a>) -> bool {
    self.state.entity(obj.key()).is_some_and(|entity| entity.frozen)
  }

  /// Whether a change of an array that the analysis knows has an effect: included code can see
  /// it.
  pub fn array_write_effects(&self, arr: &ArrValue<'a>) -> bool {
    self.is_array_escaped(arr)
      || match arr.holder {
        Some(holder) => self.state.is_included(holder),
        // A change of the length is visible to each read of the array.
        None => self
          .state
          .entity(arr.key())
          .is_some_and(|entity| entity.demand.whole || !entity.demand.children.is_empty()),
      }
  }

  /// A method call changes the elements and the length of the array that `expr` holds.
  pub fn deopt_array_change(&mut self, expr: &'a Expression<'a>, cx: Cx<'a>) {
    self.deopt_path(expr, &[Some(CompactStr::new("length"))], cx.site, 0);
  }

  /// Make the properties that a write or a `delete` of `member` can change unknown. It follows
  /// every way that `value` can find an object below `member.object()`, like rollup's
  /// `deoptimizePath`. Unlike `value`, it ignores init safety: a write that can run before the
  /// initialization can also run after it.
  pub fn deopt_member_write(&mut self, member: &'a MemberExpression<'a>, site: Site) {
    let key = self.member_key(member, site);
    self.deopt_path(member.object(), &[key], site, 0);
  }

  /// The deopt of `delete member`: a write deopt that also marks the property as removable.
  pub fn deopt_member_delete(&mut self, member: &'a MemberExpression<'a>, site: Site) {
    let deleting = std::mem::replace(&mut self.state.deleting, true);
    self.deopt_member_write(member, site);
    self.state.deleting = deleting;
  }

  /// `path` is a list of property keys below `expr`. `None` is an unknown key.
  fn deopt_path(
    &mut self,
    expr: &'a Expression<'a>,
    path: &[Option<CompactStr>],
    site: Site,
    depth: u32,
  ) {
    if depth >= MAX_DEPTH {
      self.state.deopt_all_objects();
      return;
    }
    let depth = depth + 1;
    match expr {
      Expression::ParenthesizedExpression(paren) => {
        self.deopt_path(&paren.expression, path, site, depth);
      }
      Expression::SequenceExpression(seq) => {
        if let Some(last) = seq.expressions.last() {
          self.deopt_path(last, path, site, depth);
        }
      }
      Expression::ConditionalExpression(cond) => {
        let test = self.value(&cond.test, site).truthy();
        if test != Some(false) {
          self.deopt_path(&cond.consequent, path, site, depth);
        }
        if test != Some(true) {
          self.deopt_path(&cond.alternate, path, site, depth);
        }
      }
      Expression::LogicalExpression(logical) => {
        let left = self.value(&logical.left, site);
        let left_decides = match logical.operator {
          LogicalOperator::And => left.truthy().map(|truthy| !truthy),
          LogicalOperator::Or => left.truthy(),
          LogicalOperator::Coalesce => left.nullish().map(|nullish| !nullish),
        };
        if left_decides != Some(false) {
          self.deopt_path(&logical.left, path, site, depth);
        }
        if left_decides != Some(true) {
          self.deopt_path(&logical.right, path, site, depth);
        }
      }
      Expression::Identifier(ident) => match self.resolve(site.module, ident) {
        Resolved::Root(symbol_id) => {
          let canonical = self.env.canonical(site.module, symbol_id);
          self.deopt_binding(canonical, path, depth);
        }
        Resolved::Local(symbol_id) => self.deopt_local((site.module, symbol_id), path, depth),
        Resolved::Global | Resolved::None => {}
      },
      Expression::ObjectExpression(obj) => {
        self.deopt_object(ObjValue { site, node: obj, holder: None }, path, depth);
      }
      Expression::ArrayExpression(arr) => {
        self.deopt_array(ArrValue { site, node: arr, holder: None }, path, depth);
      }
      Expression::FunctionExpression(func) => self.deopt_fn((site.module, func.node_id()), path),
      Expression::ArrowFunctionExpression(arrow) => {
        self.deopt_fn((site.module, arrow.node_id()), path);
      }
      Expression::ClassExpression(class) => {
        self.deopt_class(ClassValue { site, node: class, holder: None }, path, depth);
      }
      Expression::CallExpression(call) => {
        if let Value::Function(func) = self.value(&call.callee, site) {
          self.deopt_returns(func, path, depth);
        }
      }
      oxc::ast::match_member_expression!(Expression) => {
        let member = expr.to_member_expression();
        let key = self.member_key(member, site);
        let path = std::iter::once(key).chain(path.iter().cloned()).collect::<Vec<_>>();
        self.deopt_path(member.object(), &path, site, depth);
      }
      // Other values are untracked, or escaped where they came from.
      _ => {}
    }
  }

  fn deopt_binding(&mut self, canonical: SymbolRef, path: &[Option<CompactStr>], depth: u32) {
    let env = self.env;
    if let Some(module) = env.namespace_module(canonical) {
      // A write to a member of a namespace object throws, so only a deeper path matters.
      let Some((first, rest)) = path.split_first() else {
        return;
      };
      if rest.is_empty() {
        return;
      }
      let exports = match first {
        Some(key) => env.namespace_export(module, key).into_iter().collect::<Vec<_>>(),
        None => env.metas[module]
          .sorted_and_non_ambiguous_resolved_exports
          .keys()
          .filter_map(|name| env.namespace_export(module, name))
          .collect(),
      };
      for export in exports {
        self.deopt_binding(export, rest, depth + 1);
      }
      return;
    }
    let Some(decl) = env.decl(canonical) else {
      return;
    };
    let decl_site = Site { module: canonical.owner, stmt: decl.stmt, in_function: false };
    let (init, pattern) = match decl.init {
      Init::Expr(init) => (init, None),
      Init::Pattern(init, pattern) => (init, Some(pattern)),
      Init::Function(func) => {
        self.deopt_fn((canonical.owner, func.node_id()), path);
        return;
      }
      Init::Class(class) => {
        if self.state.active_deopts.insert(canonical) {
          let class = ClassValue { site: decl_site, node: class, holder: Some(canonical) };
          self.deopt_class(class, path, depth);
          self.state.active_deopts.remove(&canonical);
        }
        return;
      }
      Init::None => return,
    };
    if !self.state.active_deopts.insert(canonical) {
      return;
    }
    let mut full = pattern_prefix(pattern, canonical.symbol);
    full.extend(path.iter().cloned());
    self.deopt_path(init, &full, decl_site, depth);
    self.state.active_deopts.remove(&canonical);
  }

  /// A write below a binding inside a function changes the objects that its value can be: the
  /// arguments of a parameter, or the initializer of a local.
  fn deopt_local(&mut self, key: LocalKey, path: &[Option<CompactStr>], depth: u32) {
    if let Some(param) = self.state.params.get(&key).copied() {
      // A rest parameter is an array that the analysis does not model. A write through it
      // writes an unknown value, which has an effect, and the walk of a read of it gives its
      // arguments `whole`.
      if !param.rest {
        let mut full = pattern_prefix(param.pattern, key.1);
        full.extend(path.iter().cloned());
        self.deopt_args((param.module, param.func.node_id()), param.index, &full, depth);
      }
      return;
    }
    if let Some(func) = self.state.local_fns.get(&key).copied() {
      self.deopt_fn(func.key(), path);
      return;
    }
    let Some(local) = self.state.local_inits.get(&key).copied() else {
      return;
    };
    let symbol_ref = SymbolRef::from(key);
    if !self.state.active_deopts.insert(symbol_ref) {
      return;
    }
    let mut full = pattern_prefix(local.pattern, key.1);
    full.extend(path.iter().cloned());
    self.deopt_path(local.init, &full, local.site, depth);
    self.state.active_deopts.remove(&symbol_ref);
  }

  fn deopt_args(&mut self, func: NodeKey, index: usize, path: &[Option<CompactStr>], depth: u32) {
    if !self.state.add_arg_deopt(func, index, path) {
      return;
    }
    let records = self
      .state
      .calls
      .get(&func)
      .map(|calls| calls.values().copied().collect::<Vec<_>>())
      .unwrap_or_default();
    for record in records {
      self.deopt_call_arg(&record, index, path, depth);
    }
  }

  fn deopt_call_arg(
    &mut self,
    record: &CallRecord<'a>,
    index: usize,
    path: &[Option<CompactStr>],
    depth: u32,
  ) {
    for (i, arg) in record.args.iter().enumerate() {
      match arg {
        // The argument is an element of a spread value, which the analysis does not model.
        Argument::SpreadElement(spread) => {
          let full = std::iter::once(None).chain(path.iter().cloned()).collect::<Vec<_>>();
          self.deopt_path(&spread.argument, &full, record.site, depth);
          return;
        }
        arg if i == index => {
          self.deopt_path(arg.to_expression(), path, record.site, depth);
          return;
        }
        _ => {}
      }
    }
  }

  fn deopt_object(&mut self, obj: ObjValue<'a>, path: &[Option<CompactStr>], depth: u32) {
    let Some((first, rest)) = path.split_first() else {
      return;
    };
    // A read of a key that the literal does not define finds the prototype, so a deeper write
    // or a write with an unknown key can change an object below the prototype.
    if first.is_none() || !rest.is_empty() {
      let proto = obj.node.properties.iter().find_map(|prop| match prop {
        ObjectPropertyKind::ObjectProperty(prop) if is_proto_setter(prop) => Some(&prop.value),
        _ => None,
      });
      if let Some(proto) = proto {
        self.deopt_path(proto, path, obj.site, depth);
      }
    }
    let Some(key) = first.as_deref().filter(|key| *key != "__proto__") else {
      self.state.escape_entity(obj.key());
      return;
    };
    if rest.is_empty() {
      self.state.deopt_entity_key(obj.key(), Some(key));
      return;
    }
    // A deeper write changes the object at the property, not the property. A getter at the
    // key returns that object.
    for prop in &obj.node.properties {
      let ObjectPropertyKind::ObjectProperty(prop) = prop else {
        continue;
      };
      if is_proto_setter(prop) || self.property_key(prop, obj.site).is_some_and(|name| name != key)
      {
        continue;
      }
      match prop.kind {
        PropertyKind::Init => self.deopt_path(&prop.value, rest, obj.site, depth),
        PropertyKind::Get => {
          if let Some(getter) = accessor_fn(&obj, prop) {
            self.deopt_returns(getter, rest, depth);
          }
        }
        PropertyKind::Set => {}
      }
    }
  }

  /// A write below a value that a call of `func` returns changes the objects that the call can
  /// return.
  fn deopt_returns(&mut self, func: FnValue<'a>, path: &[Option<CompactStr>], depth: u32) {
    let site = func.body_site();
    let returned = match func.node.expression_body() {
      Some(expr) => vec![expr],
      None => {
        let mut returned = Vec::new();
        collect::return_arguments(func.node.body_statements(), &mut returned);
        returned
      }
    };
    for expr in returned {
      self.deopt_path(expr, path, site, depth);
    }
  }

  /// A write to a function changes the property, which can be `prototype`. The analysis does not
  /// track the values of the other properties, so a deeper write changes nothing that it
  /// tracks. A write with an unknown key or to `__proto__`, a deeper write that can change the
  /// prototype of the `prototype`, and a `delete` make the function escape.
  fn deopt_fn(&mut self, func: NodeKey, path: &[Option<CompactStr>]) {
    match path {
      [] => {}
      [Some(key), rest @ ..] if key != "__proto__" && !self.state.deleting => {
        if rest.is_empty() {
          self.state.deopt_entity_key(func, Some(key));
        } else if key == "prototype" && rest[0].as_deref().is_none_or(|key| key == "__proto__") {
          self.state.escape_entity(func);
        }
      }
      _ => self.state.escape_entity(func),
    }
  }

  /// Like `deopt_fn`, for a class. A deeper write below a static property changes the value of
  /// the static field, or of the property that the class inherits.
  fn deopt_class(&mut self, class: ClassValue<'a>, path: &[Option<CompactStr>], depth: u32) {
    if depth >= MAX_DEPTH {
      self.state.deopt_all_objects();
      return;
    }
    let entity = class.key();
    let (key, rest) = match path {
      [] => return,
      [Some(key), rest @ ..] if key != "__proto__" && !self.state.deleting => (key, rest),
      _ => {
        self.state.escape_entity(entity);
        return;
      }
    };
    if rest.is_empty() {
      self.state.deopt_entity_key(entity, Some(key));
      return;
    }
    if key == "prototype" {
      if rest[0].as_deref().is_none_or(|key| key == "__proto__") {
        self.state.escape_entity(entity);
      }
      return;
    }
    match self.class_slot(&class, key, true) {
      ClassSlot::Field(prop) => {
        if let Some(value) = &prop.value {
          self.deopt_path(value, rest, class.site, depth + 1);
        }
      }
      ClassSlot::Method(method) => self.deopt_fn((class.site.module, method.value.node_id()), rest),
      ClassSlot::Accessor { get: Some(getter), .. } => {
        let getter = FnValue {
          module: class.site.module,
          stmt: class.site.stmt,
          node: FnNode::Function(&getter.value),
          holder: class.holder,
        };
        self.deopt_returns(getter, rest, depth + 1);
      }
      ClassSlot::Accessor { get: None, .. } => {}
      ClassSlot::Missing => {
        if let Some(heritage) = class.node.heritage_expression() {
          self.deopt_path(heritage, path, class.site, depth + 1);
        }
      }
      ClassSlot::Unknown => self.state.escape_entity(entity),
    }
  }

  /// A write to an array can change its length, so it makes every element unknown. A deeper
  /// write changes the objects at the elements that its key can name.
  fn deopt_array(&mut self, arr: ArrValue<'a>, path: &[Option<CompactStr>], depth: u32) {
    let Some((first, rest)) = path.split_first() else {
      return;
    };
    let Some(key) = first.as_deref().filter(|key| *key != "__proto__") else {
      self.state.escape_entity(arr.key());
      return;
    };
    if rest.is_empty() {
      self.state.deopt_entity_key(arr.key(), None);
      return;
    }
    let Some(index) = array_index(key) else {
      return;
    };
    let has_spread = arr
      .node
      .elements
      .iter()
      .any(|element| matches!(element, ArrayExpressionElement::SpreadElement(_)));
    for (i, element) in arr.node.elements.iter().enumerate() {
      match element {
        ArrayExpressionElement::SpreadElement(spread) => {
          let full = std::iter::once(None).chain(rest.iter().cloned()).collect::<Vec<_>>();
          self.deopt_path(&spread.argument, &full, arr.site, depth);
        }
        ArrayExpressionElement::Elision(_) => {}
        element if has_spread || i == index => {
          self.deopt_path(element.to_expression(), rest, arr.site, depth);
        }
        _ => {}
      }
    }
  }

  /// The canonical top-level binding at the root of a member chain.
  fn member_root(&self, mut expr: &'a Expression<'a>, module: ModuleIdx) -> Option<SymbolRef> {
    loop {
      match expr.without_parentheses() {
        Expression::Identifier(ident) => {
          return match self.resolve(module, ident) {
            Resolved::Root(symbol_id) => Some(self.env.canonical(module, symbol_id)),
            _ => None,
          };
        }
        Expression::StaticMemberExpression(member) => expr = &member.object,
        Expression::ComputedMemberExpression(member) => expr = &member.object,
        _ => return None,
      }
    }
  }

  fn object_literal_effects(&mut self, obj: &'a ObjectExpression<'a>, cx: Cx<'a>) -> bool {
    for prop in &obj.properties {
      let effects = match prop {
        ObjectPropertyKind::SpreadProperty(spread) => {
          self.expr_effects(&spread.argument, cx) || {
            let value = self.value(&spread.argument, cx.site);
            self.spread_effects(&value)
          }
        }
        ObjectPropertyKind::ObjectProperty(prop) => self.property_effects(prop, cx),
      };
      if effects {
        return true;
      }
    }
    false
  }

  pub fn property_effects(&mut self, prop: &'a ObjectProperty<'a>, cx: Cx<'a>) -> bool {
    if prop.computed
      && let Some(key) = prop.key.as_expression()
      && (self.expr_effects(key, cx) || !is_primitive_key(&self.value(key, cx.site)))
    {
      return true;
    }
    self.expr_effects(&prop.value, cx)
  }

  /// A spread reads every own property. Only a getter can make the reads do something.
  fn spread_effects(&mut self, value: &Value<'a>) -> bool {
    if self.env.property_reads_always {
      return true;
    }
    match value {
      Value::Lit(_) => false,
      Value::Array(arr) => self.is_array_escaped(arr),
      Value::Object(obj) => {
        self.is_object_escaped(obj)
          || obj.node.properties.iter().any(|prop| {
            matches!(prop, ObjectPropertyKind::ObjectProperty(prop) if prop.kind == PropertyKind::Get)
          })
      }
      _ => true,
    }
  }

  /// A spread of a string or of an array literal iterates with a builtin iterator, which runs
  /// no user code.
  fn iterable_spread_effects(&mut self, arg: &'a Expression<'a>, cx: Cx<'a>) -> bool {
    if self.env.property_reads_always {
      return true;
    }
    match self.value(arg, cx.site) {
      Value::Lit(Lit::Str(_)) => false,
      Value::Array(arr) => self.is_array_escaped(&arr),
      _ => true,
    }
  }

  fn unary_effects(
    &mut self,
    unary: &'a UnaryExpression<'a>,
    expr: &'a Expression<'a>,
    cx: Cx<'a>,
  ) -> bool {
    match unary.operator {
      UnaryOperator::Delete => match unary.argument.without_parentheses().as_member_expression() {
        Some(member) => self.delete_effects(member, cx),
        None => true,
      },
      UnaryOperator::Typeof => match &unary.argument {
        // `typeof` of an undeclared global does not throw, but a binding in its TDZ does.
        Expression::Identifier(ident) => match self.resolve(cx.site.module, ident) {
          Resolved::Root(symbol_id) => {
            let canonical = self.env.canonical(cx.site.module, symbol_id);
            self.tdz_effects(canonical, cx.site)
          }
          _ => false,
        },
        arg => self.expr_effects(arg, cx),
      },
      UnaryOperator::Void | UnaryOperator::LogicalNot => self.expr_effects(&unary.argument, cx),
      _ => {
        self.expr_effects(&unary.argument, cx)
          || (!matches!(self.value(&unary.argument, cx.site), Value::Lit(_))
            && self.oxc_effects(expr, cx))
      }
    }
  }

  fn binary_effects(
    &mut self,
    binary: &'a BinaryExpression<'a>,
    expr: &'a Expression<'a>,
    cx: Cx<'a>,
  ) -> bool {
    if self.expr_effects(&binary.left, cx) || self.expr_effects(&binary.right, cx) {
      return true;
    }
    match binary.operator {
      BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => false,
      BinaryOperator::In => {
        if !is_primitive_key(&self.value(&binary.left, cx.site)) {
          return true;
        }
        match self.value(&binary.right, cx.site) {
          Value::Namespace(_) => false,
          // A proxy in the prototype chain can trap `in`.
          Value::Object(obj) => self.is_object_escaped(&obj) || has_proto_setter(obj.node),
          // `in` throws on a primitive. The analysis does not track the prototype of a function.
          _ => true,
        }
      }
      BinaryOperator::Instanceof => true,
      _ => {
        let primitives = matches!(self.value(&binary.left, cx.site), Value::Lit(_))
          && matches!(self.value(&binary.right, cx.site), Value::Lit(_));
        !primitives && self.oxc_effects(expr, cx)
      }
    }
  }

  fn logical_effects(&mut self, logical: &'a LogicalExpression<'a>, cx: Cx<'a>) -> bool {
    if self.expr_effects(&logical.left, cx) {
      return true;
    }
    let left = self.value(&logical.left, cx.site);
    let right_skipped = match logical.operator {
      LogicalOperator::And => left.truthy() == Some(false),
      LogicalOperator::Or => left.truthy() == Some(true),
      LogicalOperator::Coalesce => left.nullish() == Some(false),
    };
    !right_skipped && self.expr_effects(&logical.right, cx)
  }

  fn conditional_effects(&mut self, cond: &'a ConditionalExpression<'a>, cx: Cx<'a>) -> bool {
    if self.expr_effects(&cond.test, cx) {
      return true;
    }
    match self.value(&cond.test, cx.site).truthy() {
      Some(true) => self.expr_effects(&cond.consequent, cx),
      Some(false) => self.expr_effects(&cond.alternate, cx),
      None => self.expr_effects(&cond.consequent, cx) || self.expr_effects(&cond.alternate, cx),
    }
  }

  fn oxc_effects(&self, expr: &'a Expression<'a>, cx: Cx<'a>) -> bool {
    self
      .env
      .module(cx.site.module)
      .is_none_or(|facts| expr.may_have_side_effects(&OxcCtx { env: self.env, facts }))
  }

  fn oxc_member_effects(&self, member: &'a MemberExpression<'a>, cx: Cx<'a>) -> bool {
    self
      .env
      .module(cx.site.module)
      .is_none_or(|facts| member.may_have_side_effects(&OxcCtx { env: self.env, facts }))
  }
}
