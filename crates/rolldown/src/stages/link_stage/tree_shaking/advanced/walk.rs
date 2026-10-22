use oxc::{
  ast::{
    AstKind,
    ast::{
      Argument, ArrayExpressionElement, AssignmentExpression, AssignmentOperator, AssignmentTarget,
      BinaryExpression, BinaryOperator, BindingPattern, CallExpression, ChainElement,
      ConditionalExpression, Declaration, ExportDefaultDeclarationKind, Expression,
      IdentifierReference, IfStatement, LogicalExpression, LogicalOperator, MemberExpression,
      ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKind, SimpleAssignmentTarget,
      Statement, UnaryOperator, VariableDeclaration, VariableDeclarationKind,
    },
  },
  ast_visit::VisitJs,
};
use rolldown_common::{ModuleIdx, SymbolRef};

use crate::types::tree_shaking_rewrites::{FoldedExpr, FoldedIf, TreeShakingRewrites};

use super::{
  Analyzer,
  collect::{self, Excluded},
  effects::Cx,
  env::Site,
  facts::PathTree,
  value::{FnNode, FnValue, Lit, Value, is_proto_setter},
};

impl Analyzer<'_, '_> {
  /// Record a use of `ident`: a read adds `demand` to its binding, a write marks the binding
  /// reassigned, and a read of a local marks the local used.
  pub fn use_ident(&mut self, module: ModuleIdx, ident: &IdentifierReference, demand: &PathTree) {
    let Some(ref_id) = ident.reference_id.get() else {
      return;
    };
    let Some(facts) = self.env.module(module) else {
      return;
    };
    let Some(symbol_id) = facts.scopes.symbol_id_for(ref_id) else {
      return;
    };
    let reference = facts.scopes.scoping().get_reference(ref_id);
    let is_write = reference.is_write();
    let is_read = reference.is_read() || !is_write;
    if self.env.symbols.is_declared_in_root_scope((module, symbol_id).into()) {
      let canonical = self.env.canonical(module, symbol_id);
      if is_write {
        self.state.mark_reassigned(canonical);
      }
      if is_read {
        self.add_demand_to(canonical, demand);
      }
    } else if is_read {
      self.state.mark_local_used((module, symbol_id));
    }
  }

  /// Add `demand` to a binding. A demand on a namespace object goes to the exports it reads.
  pub fn add_demand_to(&mut self, canonical: SymbolRef, demand: &PathTree) {
    if self.env.module(canonical.owner).is_none() || self.state.is_escaped(canonical) {
      return;
    }
    self.state.add_demand(canonical, demand);
    let env = self.env;
    let Some(module) = env.namespace_module(canonical) else {
      return;
    };
    if demand.whole {
      for name in env.metas[module].sorted_and_non_ambiguous_resolved_exports.keys() {
        if let Some(export) = env.namespace_export(module, name) {
          self.add_demand_to(export, demand);
        }
      }
    } else {
      for (key, sub) in &demand.children {
        if let Some(export) = env.namespace_export(module, key) {
          self.add_demand_to(export, sub);
        }
      }
    }
  }
}

/// Visits code that the analysis does not model. Every read in it escapes and every write in it
/// makes its binding unknown.
pub struct EscapeVisitor<'w, 's, 'a> {
  pub an: &'w mut Analyzer<'s, 'a>,
  pub module: ModuleIdx,
}

impl<'v> VisitJs<'v> for EscapeVisitor<'_, '_, '_> {
  fn enter_node(&mut self, kind: AstKind<'v>) {
    match kind {
      AstKind::IdentifierReference(ident) => {
        self.an.use_ident(self.module, ident, &PathTree::whole());
      }
      AstKind::ObjectExpression(obj) => self.an.state.escape_entity((self.module, obj.node_id())),
      _ => {}
    }
  }
}

/// Walks the included parts of one top-level statement. It records the facts that the parts
/// create, the parts to exclude, and the rewrites that exclude them.
pub struct Walker<'w, 's, 'a> {
  pub an: &'w mut Analyzer<'s, 'a>,
  pub module: ModuleIdx,
  pub site: Site,
  pub excluded: Vec<Excluded<'a>>,
  pub rewrites: TreeShakingRewrites,
}

impl<'s, 'a> Walker<'_, 's, 'a> {
  fn escape(&mut self) -> EscapeVisitor<'_, 's, 'a> {
    EscapeVisitor { an: &mut *self.an, module: self.module }
  }

  fn removal_cx(&self) -> Cx {
    Cx::removal(self.site)
  }

  fn exclude_stmt(&mut self, stmt: &'a Statement<'a>) {
    self.excluded.push(Excluded::Stmt(stmt));
  }

  fn exclude_expr(&mut self, expr: &'a Expression<'a>) {
    self.excluded.push(Excluded::Expr(expr));
  }

  /// The demand on the value of a binding. A reassigned binding can hold values that the
  /// analysis does not see, so its initializer escapes.
  fn binding_demand(&self, holder: SymbolRef) -> PathTree {
    if self.an.state.is_reassigned(holder) {
      PathTree::whole()
    } else {
      self.an.state.demand(holder)
    }
  }

  pub fn walk_top(&mut self, stmt: &'a Statement<'a>) {
    match stmt {
      Statement::VariableDeclaration(decl) => self.walk_top_var(decl),
      Statement::FunctionDeclaration(func) => self.walk_function(FnNode::Function(func)),
      Statement::ClassDeclaration(class) => self.escape().visit_class(class),
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::VariableDeclaration(decl) => self.walk_top_var(decl),
        Declaration::FunctionDeclaration(func) => self.walk_function(FnNode::Function(func)),
        decl => self.escape().visit_declaration(decl),
      },
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
          self.walk_function(FnNode::Function(func));
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => self.escape().visit_class(class),
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {}
        expr => {
          let Some(facts) = self.an.env.module(self.module) else {
            return;
          };
          let holder = self.an.env.canonical(self.module, facts.module.default_export_ref.symbol);
          let demand = self.binding_demand(holder);
          self.walk_expr(expr.to_expression(), &demand);
        }
      },
      Statement::ImportDeclaration(_)
      | Statement::ExportAllDeclaration(_)
      | Statement::ExportNamedDeclaration(_)
      | Statement::ExportFromDeclaration(_) => {}
      stmt => self.walk_stmt(stmt),
    }
  }

  fn walk_top_var(&mut self, decl: &'a VariableDeclaration<'a>) {
    let using =
      matches!(decl.kind, VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing);
    for declarator in &decl.declarations {
      match (&declarator.id, &declarator.init) {
        (BindingPattern::BindingIdentifier(id), Some(init)) if !using => match id.symbol_id.get() {
          Some(symbol_id) => {
            let holder = self.an.env.canonical(self.module, symbol_id);
            let demand = self.binding_demand(holder);
            self.walk_expr(init, &demand);
          }
          None => self.walk_expr(init, &PathTree::whole()),
        },
        (BindingPattern::BindingIdentifier(_), None) => {}
        (pattern, init) => {
          self.escape().visit_binding_pattern(pattern);
          if let Some(init) = init {
            self.walk_expr(init, &PathTree::whole());
          }
        }
      }
    }
  }

  /// Keep the properties on a demanded path and the properties with effects. An escaped object
  /// keeps everything. `demand` must cover every read of the objects that the literal creates:
  /// a context that loses track of the value gives `whole`.
  fn walk_object(&mut self, obj: &'a ObjectExpression<'a>, demand: &PathTree) {
    let key = (self.module, obj.node_id());
    let escaped = demand.whole
      || self.an.state.all_objects_deopted
      || self.an.state.entity(key).is_some_and(|entity| entity.escaped)
      || self.an.object_uses_this(self.module, obj);
    if escaped {
      self.an.state.escape_entity(key);
    }
    for prop in &obj.properties {
      let p = match prop {
        ObjectPropertyKind::SpreadProperty(spread) => {
          self.walk_expr(&spread.argument, &PathTree::whole());
          continue;
        }
        ObjectPropertyKind::ObjectProperty(p) => p,
      };
      if escaped || is_proto_setter(p) {
        self.walk_property(p, &PathTree::whole());
        continue;
      }
      let Some(name) = self.an.property_key(p, self.site) else {
        self.walk_property(p, &PathTree::whole());
        continue;
      };
      let sub = demand.at(&name);
      let deopted = self.an.state.entity(key).is_some_and(|entity| {
        entity.all_keys_deopted || entity.deopted_keys.contains(name.as_str())
      });
      // A write runs a setter, and a write to a getter-only property throws.
      let needed = sub.is_some() || (p.kind != PropertyKind::Init && deopted);
      if needed {
        self.walk_property(p, &sub.unwrap_or_default());
      } else if self.an.property_effects(p, self.removal_cx()) {
        self.walk_property(p, &PathTree::default());
      } else {
        self.excluded.push(Excluded::Prop(prop));
        self.rewrites.removed_props.insert(p.node_id());
      }
    }
  }

  fn walk_property(&mut self, p: &'a ObjectProperty<'a>, demand: &PathTree) {
    if p.computed
      && let Some(key) = p.key.as_expression()
    {
      let key_demand = if matches!(self.an.value(key, self.site), Value::Lit(_)) {
        PathTree::default()
      } else {
        PathTree::whole()
      };
      self.walk_expr(key, &key_demand);
    }
    self.walk_expr(&p.value, demand);
  }

  pub fn walk_function(&mut self, func: FnNode<'a>) {
    let saved = self.site;
    self.site = saved.in_function();
    let params = func.params();
    for item in &params.items {
      if !matches!(item.pattern, BindingPattern::BindingIdentifier(_)) {
        self.escape().visit_binding_pattern(&item.pattern);
      }
      if let Some(init) = &item.initializer {
        self.walk_expr(init, &PathTree::whole());
      }
    }
    if let Some(rest) = &params.rest
      && !matches!(rest.rest.argument, BindingPattern::BindingIdentifier(_))
    {
      self.escape().visit_binding_pattern(&rest.rest.argument);
    }
    match func.expression_body() {
      Some(expr) => self.walk_expr(expr, &PathTree::whole()),
      None => self.walk_stmts(func.body_statements()),
    }
    self.site = saved;
  }

  /// Walk a nested statement list, and remove the statements that do nothing. Declarations,
  /// `return`, and jumps always stay.
  fn walk_stmts(&mut self, stmts: &'a [Statement<'a>]) {
    for stmt in stmts {
      let removable = match stmt {
        Statement::ExpressionStatement(_) | Statement::EmptyStatement(_) => true,
        Statement::BlockStatement(_) | Statement::IfStatement(_) => {
          !collect::contains_var_decl(stmt)
        }
        _ => false,
      };
      if removable && !self.an.stmt_effects(stmt, self.removal_cx()) {
        self.exclude_stmt(stmt);
        self.rewrites.removed_stmts.insert(stmt.node_id());
        continue;
      }
      self.walk_stmt(stmt);
    }
  }

  fn walk_stmt(&mut self, stmt: &'a Statement<'a>) {
    match stmt {
      Statement::ExpressionStatement(stmt) => {
        self.walk_expr(&stmt.expression, &PathTree::default());
      }
      Statement::BlockStatement(block) => self.walk_stmts(&block.body),
      Statement::IfStatement(if_stmt) => self.walk_if(if_stmt),
      Statement::ReturnStatement(ret) => {
        if let Some(arg) = &ret.argument {
          self.walk_expr(arg, &PathTree::whole());
        }
      }
      Statement::ThrowStatement(throw) => self.walk_expr(&throw.argument, &PathTree::whole()),
      Statement::VariableDeclaration(decl) => {
        for declarator in &decl.declarations {
          if !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
            self.escape().visit_binding_pattern(&declarator.id);
          }
          if let Some(init) = &declarator.init {
            self.walk_expr(init, &PathTree::whole());
          }
        }
      }
      Statement::FunctionDeclaration(func) => self.walk_function(FnNode::Function(func)),
      Statement::ForStatement(for_stmt) => {
        if let Some(init) = &for_stmt.init {
          match init.as_expression() {
            Some(expr) => self.walk_expr(expr, &PathTree::whole()),
            None => self.escape().visit_for_statement_init(init),
          }
        }
        if let Some(test) = &for_stmt.test {
          self.walk_expr(test, &PathTree::default());
        }
        if let Some(update) = &for_stmt.update {
          self.walk_expr(update, &PathTree::default());
        }
        self.walk_branch(&for_stmt.body);
      }
      Statement::ForInStatement(for_in) => {
        self.escape().visit_for_statement_left(&for_in.left);
        self.walk_expr(&for_in.right, &PathTree::whole());
        self.walk_branch(&for_in.body);
      }
      Statement::ForOfStatement(for_of) => {
        self.escape().visit_for_statement_left(&for_of.left);
        self.walk_expr(&for_of.right, &PathTree::whole());
        self.walk_branch(&for_of.body);
      }
      Statement::WhileStatement(while_stmt) => {
        self.walk_expr(&while_stmt.test, &PathTree::default());
        self.walk_branch(&while_stmt.body);
      }
      Statement::DoWhileStatement(do_while) => {
        self.walk_branch(&do_while.body);
        self.walk_expr(&do_while.test, &PathTree::default());
      }
      Statement::SwitchStatement(switch) => {
        self.walk_expr(&switch.discriminant, &PathTree::default());
        for case in &switch.cases {
          if let Some(test) = &case.test {
            self.walk_expr(test, &PathTree::default());
          }
          self.walk_stmts(&case.consequent);
        }
      }
      Statement::TryStatement(try_stmt) => {
        self.walk_stmts(&try_stmt.block.body);
        if let Some(handler) = &try_stmt.handler {
          if let Some(param) = &handler.param {
            self.escape().visit_binding_pattern(&param.pattern);
          }
          self.walk_stmts(&handler.body.body);
        }
        if let Some(finalizer) = &try_stmt.finalizer {
          self.walk_stmts(&finalizer.body);
        }
      }
      Statement::LabeledStatement(labeled) => self.walk_branch(&labeled.body),
      Statement::BreakStatement(_)
      | Statement::ContinueStatement(_)
      | Statement::DebuggerStatement(_)
      | Statement::EmptyStatement(_) => {}
      stmt => self.escape().visit_statement(stmt),
    }
  }

  /// A single statement in a branch or a loop body.
  fn walk_branch(&mut self, stmt: &'a Statement<'a>) {
    match stmt {
      Statement::BlockStatement(block) => self.walk_stmts(&block.body),
      stmt => self.walk_stmt(stmt),
    }
  }

  fn walk_if(&mut self, if_stmt: &'a IfStatement<'a>) {
    if let Some(test) = self.an.value(&if_stmt.test, self.site).truthy() {
      let dead = if test { if_stmt.alternate.as_ref() } else { Some(&if_stmt.consequent) };
      // Removing a `var` declaration changes the scope.
      if dead.is_none_or(|dead| !collect::contains_var_decl(dead)) {
        let keep_test = self.an.expr_effects(&if_stmt.test, self.removal_cx());
        self
          .rewrites
          .folded_ifs
          .insert(if_stmt.node_id(), FoldedIf { keep_consequent: test, keep_test });
        if keep_test {
          self.walk_expr(&if_stmt.test, &PathTree::default());
        } else {
          self.exclude_expr(&if_stmt.test);
        }
        if let Some(dead) = dead {
          self.exclude_stmt(dead);
        }
        let live = if test { Some(&if_stmt.consequent) } else { if_stmt.alternate.as_ref() };
        if let Some(live) = live {
          self.walk_branch(live);
        }
        return;
      }
    }
    self.walk_expr(&if_stmt.test, &PathTree::default());
    self.walk_branch(&if_stmt.consequent);
    if let Some(alternate) = &if_stmt.alternate {
      self.walk_branch(alternate);
    }
  }

  /// Walk an expression whose value the context uses at the paths in `demand`.
  pub fn walk_expr(&mut self, expr: &'a Expression<'a>, demand: &PathTree) {
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::StringLiteral(_)
      | Expression::ThisExpression(_)
      | Expression::Super(_)
      | Expression::ImportMeta(_)
      | Expression::NewTarget(_) => {}
      Expression::Identifier(ident) => self.an.use_ident(self.module, ident, demand),
      Expression::ParenthesizedExpression(paren) => self.walk_expr(&paren.expression, demand),
      oxc::ast::match_member_expression!(Expression) => {
        self.walk_member(expr.to_member_expression(), demand);
      }
      Expression::CallExpression(call) => self.walk_call(call, true),
      Expression::NewExpression(new) => {
        self.walk_expr(&new.callee, &PathTree::whole());
        self.walk_arguments(&new.arguments);
      }
      Expression::ChainExpression(chain) => match &chain.expression {
        ChainElement::CallExpression(call) => self.walk_call(call, false),
        ChainElement::TSNonNullExpression(inner) => self.walk_expr(&inner.expression, demand),
        element => self.walk_member(element.to_member_expression(), demand),
      },
      Expression::UnaryExpression(unary) => match unary.operator {
        UnaryOperator::Typeof | UnaryOperator::Void | UnaryOperator::LogicalNot => {
          self.walk_expr(&unary.argument, &PathTree::default());
        }
        UnaryOperator::Delete => self.walk_delete(&unary.argument),
        _ => self.walk_expr(&unary.argument, &PathTree::whole()),
      },
      Expression::BinaryExpression(binary) => self.walk_binary(binary, expr),
      Expression::LogicalExpression(logical) => self.walk_logical(logical, demand),
      Expression::ConditionalExpression(cond) => self.walk_conditional(cond, demand),
      Expression::SequenceExpression(seq) => {
        if let Some((last, rest)) = seq.expressions.split_last() {
          for expr in rest {
            self.walk_expr(expr, &PathTree::default());
          }
          self.walk_expr(last, demand);
        }
      }
      Expression::AssignmentExpression(assign) => self.walk_assignment(assign),
      Expression::UpdateExpression(update) => match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(ident) => {
          self.an.use_ident(self.module, ident, &PathTree::whole());
        }
        target @ oxc::ast::match_member_expression!(SimpleAssignmentTarget) => {
          self.walk_member_write(target.to_member_expression(), true);
        }
        target => self.escape().visit_simple_assignment_target(target),
      },
      Expression::ObjectExpression(obj) => self.walk_object(obj, demand),
      Expression::ArrayExpression(arr) => {
        for element in &arr.elements {
          match element {
            ArrayExpressionElement::SpreadElement(spread) => {
              self.walk_expr(&spread.argument, &PathTree::whole());
            }
            ArrayExpressionElement::Elision(_) => {}
            _ => self.walk_expr(element.to_expression(), &PathTree::whole()),
          }
        }
      }
      Expression::TemplateLiteral(template) => {
        for expr in &template.expressions {
          self.walk_expr(expr, &PathTree::whole());
        }
      }
      Expression::TaggedTemplateExpression(tagged) => {
        self.walk_expr(&tagged.tag, &PathTree::whole());
        for expr in &tagged.quasi.expressions {
          self.walk_expr(expr, &PathTree::whole());
        }
      }
      Expression::FunctionExpression(func) => self.walk_function(FnNode::Function(func)),
      Expression::ArrowFunctionExpression(arrow) => self.walk_function(FnNode::Arrow(arrow)),
      Expression::AwaitExpression(await_expr) => {
        self.walk_expr(&await_expr.argument, &PathTree::whole());
      }
      Expression::YieldExpression(yield_expr) => {
        if let Some(arg) = &yield_expr.argument {
          self.walk_expr(arg, &PathTree::whole());
        }
      }
      expr => self.escape().visit_expression(expr),
    }
  }

  fn walk_member(&mut self, member: &'a MemberExpression<'a>, demand: &PathTree) {
    match member {
      MemberExpression::StaticMemberExpression(member) => {
        let object_demand = PathTree::prefixed(member.property.name.as_str(), demand.clone());
        self.walk_expr(&member.object, &object_demand);
      }
      MemberExpression::ComputedMemberExpression(member) => {
        match self.an.computed_key(&member.expression, self.site) {
          Some(key) => {
            self.walk_expr(&member.expression, &PathTree::default());
            self.walk_expr(&member.object, &PathTree::prefixed(&key, demand.clone()));
          }
          None => {
            self.walk_expr(&member.expression, &PathTree::whole());
            self.walk_expr(&member.object, &PathTree::whole());
          }
        }
      }
      MemberExpression::PrivateFieldExpression(member) => {
        self.walk_expr(&member.object, &PathTree::whole());
      }
    }
  }

  /// Walk a call. A known callee needs only the property that holds it, unless it reads
  /// `this`. An argument goes when the callee never reads its parameter and the argument does
  /// nothing.
  fn walk_call(&mut self, call: &'a CallExpression<'a>, can_remove_args: bool) {
    let callee_fn = match self.an.value(&call.callee, self.site) {
      Value::Function(func) => Some(func),
      _ => None,
    };
    match call.callee.without_parentheses() {
      callee @ oxc::ast::match_member_expression!(Expression) => {
        let member = callee.to_member_expression();
        let key = self.an.member_key(member, self.site);
        let this_free = match &callee_fn {
          Some(func) => !self.an.fn_uses_this(func),
          None => false,
        };
        match (member, key) {
          (MemberExpression::PrivateFieldExpression(member), _) => {
            self.walk_expr(&member.object, &PathTree::whole());
          }
          (member, Some(key)) if this_free => {
            if let MemberExpression::ComputedMemberExpression(computed) = member {
              self.walk_expr(&computed.expression, &PathTree::default());
            }
            self.walk_expr(member.object(), &PathTree::prefixed(&key, PathTree::default()));
          }
          (member, _) => {
            if let MemberExpression::ComputedMemberExpression(computed) = member {
              self.walk_expr(&computed.expression, &PathTree::whole());
            }
            self.walk_expr(member.object(), &PathTree::whole());
          }
        }
      }
      _ => self.walk_expr(&call.callee, &PathTree::default()),
    }
    let removable = callee_fn.filter(|func| {
      can_remove_args
        && !call.arguments.iter().any(|arg| matches!(arg, Argument::SpreadElement(_)))
        && !self.an.fn_uses_arguments(func)
    });
    for (index, arg) in call.arguments.iter().enumerate() {
      let Some(expr) = arg.as_expression() else {
        if let Argument::SpreadElement(spread) = arg {
          self.walk_expr(&spread.argument, &PathTree::whole());
        }
        continue;
      };
      if let Some(func) = &removable
        && self.is_param_unused(func, index)
        && !self.an.expr_effects(expr, self.removal_cx())
      {
        self.excluded.push(Excluded::Arg(arg));
        self.rewrites.removed_args.insert(arg.node_id());
        continue;
      }
      self.walk_expr(expr, &PathTree::whole());
    }
  }

  fn walk_arguments(&mut self, arguments: &'a [Argument<'a>]) {
    for arg in arguments {
      match arg {
        Argument::SpreadElement(spread) => self.walk_expr(&spread.argument, &PathTree::whole()),
        _ => self.walk_expr(arg.to_expression(), &PathTree::whole()),
      }
    }
  }

  fn is_param_unused(&self, func: &FnValue<'a>, index: usize) -> bool {
    let params = func.node.params();
    match params.items.get(index) {
      Some(item) => {
        item.initializer.is_none()
          && match &item.pattern {
            BindingPattern::BindingIdentifier(id) => id.symbol_id.get().is_some_and(|symbol_id| {
              !self.an.state.used_locals.contains(&(func.module, symbol_id))
            }),
            _ => false,
          }
      }
      None => params.rest.is_none(),
    }
  }

  fn walk_binary(&mut self, binary: &'a BinaryExpression<'a>, expr: &'a Expression<'a>) {
    if binary.operator == BinaryOperator::In {
      // `'x' in ns` must not build the namespace object.
      if let Value::Namespace(_) = self.an.value(&binary.right, self.site)
        && let Value::Lit(Lit::Bool(has)) = self.an.value(expr, self.site)
        && !self.an.expr_effects(expr, self.removal_cx())
      {
        self.rewrites.folded_exprs.insert(binary.node_id(), FoldedExpr::Bool(has));
        self.exclude_expr(expr);
        return;
      }
      let key = self.an.computed_key(&binary.left, self.site);
      let left_demand = if key.is_some() { PathTree::default() } else { PathTree::whole() };
      self.walk_expr(&binary.left, &left_demand);
      let right_demand = match key {
        Some(key) => PathTree::prefixed(&key, PathTree::default()),
        None => PathTree::whole(),
      };
      self.walk_expr(&binary.right, &right_demand);
      return;
    }
    let (left, right) = match binary.operator {
      BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => {
        (PathTree::default(), PathTree::default())
      }
      // Other operators convert objects to primitives, which can read any property. A custom
      // `Symbol.hasInstance` of the right operand of `instanceof` gets the left operand.
      _ => (PathTree::whole(), PathTree::whole()),
    };
    self.walk_expr(&binary.left, &left);
    self.walk_expr(&binary.right, &right);
  }

  fn walk_logical(&mut self, logical: &'a LogicalExpression<'a>, demand: &PathTree) {
    let left = self.an.value(&logical.left, self.site);
    let left_decides = match logical.operator {
      LogicalOperator::And => left.truthy().map(|truthy| !truthy),
      LogicalOperator::Or => left.truthy(),
      LogicalOperator::Coalesce => left.nullish().map(|nullish| !nullish),
    };
    match left_decides {
      Some(true) => {
        self.rewrites.folded_exprs.insert(logical.node_id(), FoldedExpr::Logical(true));
        self.exclude_expr(&logical.right);
        self.walk_expr(&logical.left, demand);
      }
      Some(false) if !self.an.expr_effects(&logical.left, self.removal_cx()) => {
        self.rewrites.folded_exprs.insert(logical.node_id(), FoldedExpr::Logical(false));
        self.exclude_expr(&logical.left);
        self.walk_expr(&logical.right, demand);
      }
      _ => {
        self.walk_expr(&logical.left, demand);
        self.walk_expr(&logical.right, demand);
      }
    }
  }

  fn walk_conditional(&mut self, cond: &'a ConditionalExpression<'a>, demand: &PathTree) {
    if let Some(test) = self.an.value(&cond.test, self.site).truthy()
      && !self.an.expr_effects(&cond.test, self.removal_cx())
    {
      self.rewrites.folded_exprs.insert(cond.node_id(), FoldedExpr::Conditional(test));
      self.exclude_expr(&cond.test);
      let (live, dead) = if test {
        (&cond.consequent, &cond.alternate)
      } else {
        (&cond.alternate, &cond.consequent)
      };
      self.exclude_expr(dead);
      self.walk_expr(live, demand);
      return;
    }
    self.walk_expr(&cond.test, &PathTree::default());
    self.walk_expr(&cond.consequent, demand);
    self.walk_expr(&cond.alternate, demand);
  }

  fn walk_assignment(&mut self, assign: &'a AssignmentExpression<'a>) {
    let compound = assign.operator != AssignmentOperator::Assign;
    match &assign.left {
      AssignmentTarget::AssignmentTargetIdentifier(ident) => {
        let demand = if compound { PathTree::whole() } else { PathTree::default() };
        self.an.use_ident(self.module, ident, &demand);
      }
      target @ oxc::ast::match_member_expression!(AssignmentTarget) => {
        self.walk_member_write(target.to_member_expression(), compound);
      }
      target => self.escape().visit_assignment_target(target),
    }
    self.walk_expr(&assign.right, &PathTree::whole());
  }

  fn walk_member_write(&mut self, member: &'a MemberExpression<'a>, compound: bool) {
    self.an.deopt_member_write(member, self.site);
    let key = self.an.member_key(member, self.site);
    match member {
      MemberExpression::PrivateFieldExpression(member) => {
        self.walk_expr(&member.object, &PathTree::whole());
      }
      member => {
        if let MemberExpression::ComputedMemberExpression(computed) = member {
          let key_demand = if key.is_some() { PathTree::default() } else { PathTree::whole() };
          self.walk_expr(&computed.expression, &key_demand);
        }
        // The written property must stay, because a setter or a getter-only property at it
        // makes the write do something. The value that the write reaches can be one that the
        // write does not see, so the demand is the only fact that keeps such a property.
        let object_demand = match (&key, compound) {
          (Some(key), true) => PathTree::prefixed(key, PathTree::whole()),
          (Some(key), false) => PathTree::prefixed(key, PathTree::default()),
          (None, _) => PathTree::whole(),
        };
        self.walk_expr(member.object(), &object_demand);
      }
    }
  }

  fn walk_delete(&mut self, arg: &'a Expression<'a>) {
    match arg.without_parentheses() {
      target @ oxc::ast::match_member_expression!(Expression) => {
        self.walk_member_write(target.to_member_expression(), false);
      }
      arg => self.walk_expr(arg, &PathTree::default()),
    }
  }
}
