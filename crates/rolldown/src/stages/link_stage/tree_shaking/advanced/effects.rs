use oxc::{
  ast::ast::{
    Argument, ArrayExpressionElement, AssignmentExpression, AssignmentOperator, AssignmentTarget,
    BinaryExpression, BinaryOperator, BindingPattern, CallExpression, ChainElement, Class,
    ConditionalExpression, Declaration, ExportDefaultDeclarationKind, Expression, FormalParameters,
    IdentifierReference, LogicalExpression, LogicalOperator, MemberExpression, NewExpression,
    ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKind, SimpleAssignmentTarget,
    Statement, UnaryExpression, UnaryOperator, VariableDeclaration, VariableDeclarationKind,
  },
  ast_visit::VisitJs,
  semantic::{ScopeId, SymbolId},
};
use oxc_ecmascript::side_effects::MayHaveSideEffects;
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, SymbolRef};

use super::{
  Analyzer, collect,
  env::{DeclKind, Init, OxcCtx, Site, is_manual_pure},
  value::{FnValue, Lit, MAX_DEPTH, ObjValue, Resolved, Value, has_proto_setter, is_proto_setter},
  walk::EscapeVisitor,
};

/// The context of an effect check.
#[derive(Debug, Clone, Copy)]
pub struct Cx {
  pub site: Site,
  /// The scope of the function whose call the check analyzes. A write to a `var` or a
  /// parameter of that function is not visible after the call. `None` when the check decides
  /// whether code can go, and then every write is visible.
  pub callee_scope: Option<ScopeId>,
}

impl Cx {
  pub fn removal(site: Site) -> Self {
    Self { site, callee_scope: None }
  }
}

/// The effect checker. A result of `false` means that removing the code changes nothing, so
/// the check must visit every write in the code and apply its deopts: the removal decision can
/// depend on facts that the write changes. A result of `true` keeps the code, and then the walker
/// visits it, so the check can stop early.
impl<'a> Analyzer<'_, 'a> {
  pub fn stmts_effects(&mut self, stmts: &'a [Statement<'a>], cx: Cx) -> bool {
    for stmt in stmts {
      if self.stmt_effects(stmt, cx) {
        return true;
      }
    }
    false
  }

  pub fn stmt_effects(&mut self, stmt: &'a Statement<'a>, cx: Cx) -> bool {
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
      Statement::ClassDeclaration(class) => self.oxc_class_effects(class, cx),
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
        ExportDefaultDeclarationKind::ClassDeclaration(class) => self.oxc_class_effects(class, cx),
        expr => self.expr_effects(expr.to_expression(), cx),
      },
      _ => true,
    }
  }

  fn decl_effects(&mut self, decl: &'a Declaration<'a>, cx: Cx) -> bool {
    match decl {
      Declaration::VariableDeclaration(decl) => self.var_decl_effects(decl, cx),
      Declaration::FunctionDeclaration(_)
      | Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_) => false,
      Declaration::ClassDeclaration(class) => self.oxc_class_effects(class, cx),
      _ => true,
    }
  }

  fn var_decl_effects(&mut self, decl: &'a VariableDeclaration<'a>, cx: Cx) -> bool {
    if matches!(decl.kind, VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing) {
      return true;
    }
    for declarator in &decl.declarations {
      // Destructuring can run getters and iterators.
      if !matches!(declarator.id, BindingPattern::BindingIdentifier(_))
        || declarator.init.as_ref().is_some_and(|init| self.expr_effects(init, cx))
      {
        return true;
      }
    }
    false
  }

  pub fn expr_effects(&mut self, expr: &'a Expression<'a>, cx: Cx) -> bool {
    if self.state.depth >= MAX_DEPTH {
      return true;
    }
    self.state.depth += 1;
    let effects = self.expr_effects_inner(expr, cx);
    self.state.depth -= 1;
    effects
  }

  fn expr_effects_inner(&mut self, expr: &'a Expression<'a>, cx: Cx) -> bool {
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
                || !matches!(self.value(&spread.argument, cx.site), Value::Lit(Lit::Str(_)))
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
      Expression::ClassExpression(class) => self.oxc_class_effects(class, cx),
      Expression::TaggedTemplateExpression(_)
      | Expression::PrivateInExpression(_)
      | Expression::JSXElement(_)
      | Expression::JSXFragment(_) => self.oxc_effects(expr, cx),
      _ => true,
    }
  }

  fn ident_read_effects(&self, ident: &IdentifierReference<'a>, cx: Cx) -> bool {
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

  fn member_read_effects(&mut self, member: &'a MemberExpression<'a>, cx: Cx) -> bool {
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
        if key.is_none() && !matches!(self.value(&computed.expression, cx.site), Value::Lit(_)) {
          // Converting an object to a property key can run user code.
          return true;
        }
        key
      }
      MemberExpression::PrivateFieldExpression(_) => return true,
    };
    match self.value(member.object(), cx.site) {
      Value::Lit(Lit::Undefined | Lit::Null) => !member.optional(),
      Value::Lit(_) => false,
      Value::Object(obj) => {
        if self.is_object_escaped(&obj) || has_proto_setter(obj.node) {
          self.oxc_member_effects(member, cx)
        } else {
          self.object_getter_at(&obj, key.as_deref())
        }
      }
      Value::Namespace(module) => match key.as_deref() {
        Some(key) => self
          .env
          .namespace_export(module, key)
          .is_some_and(|export| self.tdz_effects(export, cx.site)),
        None => true,
      },
      // The analysis does not track the properties of a function, and unknown code can define
      // a getter on one.
      Value::Function(_) | Value::Unknown => self.oxc_member_effects(member, cx),
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

  fn call_effects(&mut self, call: &'a CallExpression<'a>, cx: Cx) -> bool {
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
    match self.value(&call.callee, cx.site) {
      Value::Function(func) => self.function_call_effects(func),
      Value::Unknown => self
        .env
        .module(cx.site.module)
        .is_none_or(|facts| call.may_have_side_effects(&OxcCtx { env: self.env, facts })),
      Value::Lit(Lit::Undefined | Lit::Null) => !call.optional,
      // Calling a value that is not a function throws.
      _ => true,
    }
  }

  fn arguments_effects(&mut self, arguments: &'a [Argument<'a>], cx: Cx) -> bool {
    for arg in arguments {
      let effects = match arg {
        Argument::SpreadElement(spread) => {
          self.expr_effects(&spread.argument, cx)
            || !matches!(self.value(&spread.argument, cx.site), Value::Lit(Lit::Str(_)))
        }
        _ => self.expr_effects(arg.to_expression(), cx),
      };
      if effects {
        return true;
      }
    }
    false
  }

  fn new_effects(&mut self, new: &'a NewExpression<'a>, expr: &'a Expression<'a>, cx: Cx) -> bool {
    if self.expr_effects(&new.callee, cx) || self.arguments_effects(&new.arguments, cx) {
      return true;
    }
    !(new.pure && self.env.annotations()) && self.oxc_effects(expr, cx)
  }

  /// Whether a call of `func` has effects outside the call. A recursive call assumes that it
  /// has none, like rollup does.
  pub fn function_call_effects(&mut self, func: FnValue<'a>) -> bool {
    let key = func.key();
    if let Some(effects) = self.state.call_effects.get(&key) {
      return *effects;
    }
    if self.state.active_calls.contains(&key) {
      self.state.used_assumption = true;
      return false;
    }
    if self.state.depth >= MAX_DEPTH {
      return true;
    }
    self.state.active_calls.insert(key);
    let saved_assumption = std::mem::replace(&mut self.state.used_assumption, false);
    let cx = Cx { site: func.body_site(), callee_scope: func.node.scope_id() };
    // A generator call runs only the parameter defaults.
    let effects = self.params_effects(func.node.params(), cx)
      || (!func.node.is_generator()
        && match func.node.expression_body() {
          Some(expr) => self.expr_effects(expr, cx),
          None => self.stmts_effects(func.node.body_statements(), cx),
        });
    self.state.active_calls.remove(&key);
    let used_assumption = std::mem::replace(&mut self.state.used_assumption, saved_assumption);
    if used_assumption {
      self.state.used_assumption = true;
    } else {
      self.state.call_effects.insert(key, effects);
    }
    effects
  }

  fn params_effects(&mut self, params: &'a FormalParameters<'a>, cx: Cx) -> bool {
    for item in &params.items {
      if !matches!(item.pattern, BindingPattern::BindingIdentifier(_))
        || item.initializer.as_ref().is_some_and(|init| self.expr_effects(init, cx))
      {
        return true;
      }
    }
    params
      .rest
      .as_ref()
      .is_some_and(|rest| !matches!(rest.rest.argument, BindingPattern::BindingIdentifier(_)))
  }

  fn assignment_effects(&mut self, assign: &'a AssignmentExpression<'a>, cx: Cx) -> bool {
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

  fn update_deopts(&mut self, target: &'a SimpleAssignmentTarget<'a>, cx: Cx) {
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
    cx: Cx,
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
      Resolved::Local(symbol_id) => compound || !self.is_callee_var(cx, symbol_id),
      Resolved::Global | Resolved::None => true,
    }
  }

  /// Whether `symbol_id` is a `var` or a parameter of the function whose call `cx` analyzes.
  fn is_callee_var(&self, cx: Cx, symbol_id: SymbolId) -> bool {
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
    cx: Cx,
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
          || (key.is_none() && !matches!(self.value(&computed.expression, cx.site), Value::Lit(_)))
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
    match object {
      Value::Object(obj) => {
        key == "__proto__"
          || self.is_object_escaped(&obj)
          || has_proto_setter(obj.node)
          // A setter runs. A write to a getter-only property throws.
          || self.object_accessor_at(&obj, Some(&key), |kind| kind != PropertyKind::Init)
          || obj.holder.is_some_and(|holder| self.state.is_included(holder))
      }
      Value::Function(func) => {
        // A new prototype can bring setters.
        matches!(
          key.as_str(),
          "name" | "length" | "prototype" | "caller" | "arguments" | "__proto__"
        ) || func.holder.is_none_or(|holder| self.state.is_included(holder))
      }
      _ => true,
    }
  }

  /// Make the properties that a write or a `delete` of `member` can change unknown. It follows
  /// every way that `value` can find an object below `member.object()`, like rollup's
  /// `deoptimizePath`. Unlike `value`, it ignores init safety: a write that can run before the
  /// initialization can also run after it.
  pub fn deopt_member_write(&mut self, member: &'a MemberExpression<'a>, site: Site) {
    let key = self.member_key(member, site);
    self.deopt_path(member.object(), &[key], site, 0);
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
      Expression::Identifier(ident) => {
        if let Resolved::Root(symbol_id) = self.resolve(site.module, ident) {
          let canonical = self.env.canonical(site.module, symbol_id);
          self.deopt_binding(canonical, path, depth);
        }
      }
      Expression::ObjectExpression(obj) => {
        self.deopt_object(ObjValue { site, node: obj, holder: None }, path, depth);
      }
      Expression::CallExpression(call) => {
        if let Value::Function(func) = self.value(&call.callee, site) {
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
    let Init::Expr(init) = decl.init else {
      return;
    };
    if !self.state.active_deopts.insert(canonical) {
      return;
    }
    let site = Site { module: canonical.owner, stmt: decl.stmt, in_function: false };
    self.deopt_path(init, path, site, depth);
    self.state.active_deopts.remove(&canonical);
  }

  fn deopt_object(&mut self, obj: ObjValue<'a>, path: &[Option<CompactStr>], depth: u32) {
    let Some((first, rest)) = path.split_first() else {
      return;
    };
    let Some(key) = first.as_deref().filter(|key| *key != "__proto__") else {
      self.state.escape_entity(obj.key());
      return;
    };
    if rest.is_empty() {
      self.state.deopt_entity_key(obj.key(), Some(key));
      return;
    }
    // A deeper write changes the object at the property, not the property.
    for prop in &obj.node.properties {
      let ObjectPropertyKind::ObjectProperty(prop) = prop else {
        continue;
      };
      if prop.kind == PropertyKind::Init
        && !is_proto_setter(prop)
        && self.property_key(prop, obj.site).is_none_or(|name| name == key)
      {
        self.deopt_path(&prop.value, rest, obj.site, depth);
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

  fn object_literal_effects(&mut self, obj: &'a ObjectExpression<'a>, cx: Cx) -> bool {
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

  pub fn property_effects(&mut self, prop: &'a ObjectProperty<'a>, cx: Cx) -> bool {
    if prop.computed
      && let Some(key) = prop.key.as_expression()
      && (self.expr_effects(key, cx) || !matches!(self.value(key, cx.site), Value::Lit(_)))
    {
      return true;
    }
    self.expr_effects(&prop.value, cx)
  }

  /// A spread reads every own property. Only a getter can make the reads do something.
  fn spread_effects(&mut self, value: &Value<'a>) -> bool {
    match value {
      Value::Lit(_) => false,
      Value::Object(obj) => {
        self.is_object_escaped(obj)
          || obj.node.properties.iter().any(|prop| {
            matches!(prop, ObjectPropertyKind::ObjectProperty(prop) if prop.kind == PropertyKind::Get)
          })
      }
      _ => true,
    }
  }

  fn unary_effects(
    &mut self,
    unary: &'a UnaryExpression<'a>,
    expr: &'a Expression<'a>,
    cx: Cx,
  ) -> bool {
    match unary.operator {
      UnaryOperator::Delete => {
        if let Some(member) = unary.argument.without_parentheses().as_member_expression() {
          self.deopt_member_write(member, cx.site);
        }
        true
      }
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
    cx: Cx,
  ) -> bool {
    if self.expr_effects(&binary.left, cx) || self.expr_effects(&binary.right, cx) {
      return true;
    }
    match binary.operator {
      BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => false,
      BinaryOperator::In => {
        if !matches!(self.value(&binary.left, cx.site), Value::Lit(_)) {
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

  fn logical_effects(&mut self, logical: &'a LogicalExpression<'a>, cx: Cx) -> bool {
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

  fn conditional_effects(&mut self, cond: &'a ConditionalExpression<'a>, cx: Cx) -> bool {
    if self.expr_effects(&cond.test, cx) {
      return true;
    }
    match self.value(&cond.test, cx.site).truthy() {
      Some(true) => self.expr_effects(&cond.consequent, cx),
      Some(false) => self.expr_effects(&cond.alternate, cx),
      None => self.expr_effects(&cond.consequent, cx) || self.expr_effects(&cond.alternate, cx),
    }
  }

  fn oxc_effects(&self, expr: &'a Expression<'a>, cx: Cx) -> bool {
    self
      .env
      .module(cx.site.module)
      .is_none_or(|facts| expr.may_have_side_effects(&OxcCtx { env: self.env, facts }))
  }

  fn oxc_member_effects(&self, member: &'a MemberExpression<'a>, cx: Cx) -> bool {
    self
      .env
      .module(cx.site.module)
      .is_none_or(|facts| member.may_have_side_effects(&OxcCtx { env: self.env, facts }))
  }

  fn oxc_class_effects(&self, class: &'a Class<'a>, cx: Cx) -> bool {
    self
      .env
      .module(cx.site.module)
      .is_none_or(|facts| class.may_have_side_effects(&OxcCtx { env: self.env, facts }))
  }
}
