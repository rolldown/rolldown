use oxc::{
  ast::{
    AstKind,
    ast::{
      Argument, ArrayExpression, ArrayExpressionElement, AssignmentExpression, AssignmentOperator,
      AssignmentTarget, BinaryExpression, BinaryOperator, BindingPattern, CallExpression,
      ChainElement, Class, ConditionalExpression, Declaration, ExportDefaultDeclarationKind,
      Expression, IdentifierReference, IfStatement, LogicalExpression, LogicalOperator,
      MemberExpression, ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKind,
      SimpleAssignmentTarget, Statement, UnaryOperator, VariableDeclaration,
      VariableDeclarationKind,
    },
  },
  ast_visit::VisitJs,
  semantic::SymbolId,
};
use rolldown_common::{ModuleIdx, SymbolRef};

use crate::types::tree_shaking_rewrites::{FoldedExpr, FoldedIf, TreeShakingRewrites};

use super::{
  Analyzer,
  collect::{self, Excluded},
  effects::Cx,
  env::Site,
  facts::PathTree,
  value::{FnNode, FnValue, Lit, Resolved, Value, is_primitive_key, is_proto_setter},
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
    } else {
      let local = SymbolRef::from((module, symbol_id));
      if is_write {
        self.state.mark_reassigned(local);
      }
      if is_read {
        self.state.mark_local_used((module, symbol_id));
        self.state.add_demand(local, demand);
      }
    }
  }

  /// A direct `eval` in included code can read and write each binding in its scope by name. The
  /// analysis does not track scopes, so every binding of the module escapes and becomes
  /// unknown. Then the statements that the `eval` bailout of the inclusion engine keeps stay too.
  pub fn on_direct_eval(&mut self, module: ModuleIdx) {
    if !self.state.mark_live_eval(module) {
      return;
    }
    let env = self.env;
    let Some(facts) = env.module(module) else {
      return;
    };
    for symbol_id in facts.scopes.scoping().symbol_ids() {
      let symbol_ref = SymbolRef::from((module, symbol_id));
      if env.symbols.is_declared_in_root_scope(symbol_ref) {
        // `eval` cannot write an import, but it can read it.
        if !facts.is_named_import(symbol_id) {
          self.state.mark_reassigned(env.canonical(module, symbol_id));
        }
        self.add_demand_to(env.canonical(module, symbol_id), &PathTree::whole());
      } else {
        self.state.mark_reassigned(symbol_ref);
        self.state.mark_local_used((module, symbol_id));
        self.state.add_demand(symbol_ref, &PathTree::whole());
      }
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
      AstKind::ArrayExpression(arr) => self.an.state.escape_entity((self.module, arr.node_id())),
      AstKind::RegExpLiteral(re) => self.an.state.escape_entity((self.module, re.node_id())),
      AstKind::Class(class) => self.an.state.escape_entity((self.module, class.node_id())),
      // Unknown code can call a function in code that the analysis does not model.
      AstKind::Function(func) => self.an.state.escape_entity((self.module, func.node_id())),
      AstKind::ArrowFunctionExpression(arrow) => {
        self.an.state.escape_entity((self.module, arrow.node_id()));
      }
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
  /// The innermost function that the walk is in.
  pub func: Option<FnValue<'a>>,
  /// The walk is in a catching `try` block (see `collect::discards_throw`), or in an argument or
  /// a function that such a block can call. The code after the block sees each throw, also a
  /// throw of code that oxc treats as pure, such as `new WeakSet([1])`. So code that runs stays
  /// there, and only code that never runs goes, like rollup's default
  /// `treeshake.tryCatchDeoptimization`. See internal-docs/advanced-tree-shaking/design.md.
  pub catching: bool,
  pub excluded: Vec<Excluded<'a>>,
  pub rewrites: TreeShakingRewrites,
}

impl<'s, 'a> Walker<'_, 's, 'a> {
  fn escape(&mut self) -> EscapeVisitor<'_, 's, 'a> {
    EscapeVisitor { an: &mut *self.an, module: self.module }
  }

  fn removal_cx(&self) -> Cx<'a> {
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

  fn symbol_demand(&self, module: ModuleIdx, symbol_id: SymbolId) -> PathTree {
    self.binding_demand(self.an.symbol_key(module, symbol_id))
  }

  /// Whether included code reads a binding of the pattern.
  fn is_pattern_read(&self, module: ModuleIdx, pattern: &BindingPattern) -> bool {
    pattern.get_binding_identifiers().iter().any(|id| {
      id.symbol_id.get().is_none_or(|symbol_id| {
        let key = self.an.symbol_key(module, symbol_id);
        if key.owner == module && !self.an.env.symbols.is_declared_in_root_scope(key) {
          self.an.state.used_locals.contains(&(module, symbol_id))
        } else {
          self.an.state.is_included(key) || self.an.state.is_reassigned(key)
        }
      })
    })
  }

  /// The demand on the value that a binding pattern of `module` destructures.
  fn pattern_demand(&mut self, module: ModuleIdx, pattern: &'a BindingPattern<'a>) -> PathTree {
    match pattern {
      BindingPattern::BindingIdentifier(id) => id
        .symbol_id
        .get()
        .map_or_else(PathTree::whole, |symbol_id| self.symbol_demand(module, symbol_id)),
      BindingPattern::ObjectPattern(obj) => {
        let mut tree = PathTree::default();
        for prop in &obj.properties {
          let Some(key) = self.an.binding_key(prop, self.site) else {
            return PathTree::whole();
          };
          let sub = self.pattern_demand(module, &prop.value);
          tree.merge(&PathTree::prefixed(&key, sub));
        }
        // A rest element copies the other properties. When code does not read the copy, the
        // rest element still reads each key, which runs the getters.
        if let Some(rest) = &obj.rest {
          if self.is_pattern_read(module, &rest.argument) {
            return PathTree::whole();
          }
          tree.every_key = true;
        }
        tree
      }
      // The iterator protocol can read anything.
      BindingPattern::ArrayPattern(_) => PathTree::whole(),
      BindingPattern::AssignmentPattern(assign) => self.pattern_demand(module, &assign.left),
    }
  }

  /// Walk the computed keys and the default values of a binding pattern. A default value flows
  /// into its binding.
  fn walk_pattern_parts(&mut self, pattern: &'a BindingPattern<'a>) {
    match pattern {
      BindingPattern::BindingIdentifier(_) => {}
      BindingPattern::ObjectPattern(obj) => {
        for prop in &obj.properties {
          if prop.computed
            && let Some(key) = prop.key.as_expression()
          {
            let key_demand = if is_primitive_key(&self.an.value(key, self.site)) {
              PathTree::default()
            } else {
              PathTree::whole()
            };
            self.walk_expr(key, &key_demand);
          }
          self.walk_pattern_parts(&prop.value);
        }
        if let Some(rest) = &obj.rest {
          self.walk_pattern_parts(&rest.argument);
        }
      }
      BindingPattern::ArrayPattern(arr) => {
        for element in arr.elements.iter().flatten() {
          self.walk_pattern_parts(element);
        }
        if let Some(rest) = &arr.rest {
          self.walk_pattern_parts(&rest.argument);
        }
      }
      BindingPattern::AssignmentPattern(assign) => {
        let demand = self.pattern_demand(self.module, &assign.left);
        self.walk_expr(&assign.right, &demand);
        self.walk_pattern_parts(&assign.left);
      }
    }
  }

  /// The demand on the values that the current function returns. Unknown code can read what an
  /// escaped function returns, and a promise or an iterator of an async function or a generator
  /// goes to code that the analysis does not model.
  fn return_demand(&self) -> PathTree {
    match self.func {
      Some(func)
        if !func.node.is_async()
          && !func.node.is_generator()
          && !self.an.is_fn_escaped(func.module, func.node) =>
      {
        self.an.state.return_demand(func.key())
      }
      _ => PathTree::whole(),
    }
  }

  /// The demand on the argument at `index` of a call of `func`: the demand on the parameter
  /// that receives it.
  fn param_demand(&mut self, func: &FnValue<'a>, index: usize) -> PathTree {
    let params = func.node.params();
    match params.items.get(index) {
      Some(item) => self.pattern_demand(func.module, &item.pattern),
      None => match &params.rest {
        // The rest parameter is an array that the analysis does not model.
        Some(rest) if self.is_pattern_read(func.module, &rest.rest.argument) => PathTree::whole(),
        _ => PathTree::default(),
      },
    }
  }

  pub fn walk_top(&mut self, stmt: &'a Statement<'a>) {
    match stmt {
      Statement::VariableDeclaration(decl) => self.walk_var_decl(decl),
      Statement::FunctionDeclaration(func) => self.walk_function(FnNode::Function(func)),
      Statement::ClassDeclaration(class) => self.walk_class_decl(class),
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::VariableDeclaration(decl) => self.walk_var_decl(decl),
        Declaration::FunctionDeclaration(func) => self.walk_function(FnNode::Function(func)),
        Declaration::ClassDeclaration(class) => self.walk_class_decl(class),
        decl => self.escape().visit_declaration(decl),
      },
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
          self.walk_function(FnNode::Function(func));
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => self.walk_class_decl(class),
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

  /// Walk a top-level class declaration with the demand on its binding.
  fn walk_class_decl(&mut self, class: &'a Class<'a>) {
    let symbol_id = class.id.as_ref().and_then(|id| id.symbol_id.get()).or_else(|| {
      self.an.env.module(self.module).map(|facts| facts.module.default_export_ref.symbol)
    });
    let demand = symbol_id
      .map_or_else(PathTree::whole, |symbol_id| self.symbol_demand(self.module, symbol_id));
    self.walk_class(class, &demand);
  }

  /// The analysis does not walk the parts of a class, so each reference in them escapes. The
  /// class itself escapes only when `demand` lets unknown code see it.
  fn walk_class(&mut self, class: &'a Class<'a>, demand: &PathTree) {
    let key = (self.module, class.node_id());
    if demand.whole {
      self.an.state.escape_entity(key);
    } else {
      self.an.state.add_entity_demand(key, demand);
    }
    let mut escape = self.escape();
    escape.visit_decorators(&class.decorators);
    if let Some(heritage) = &class.heritage {
      escape.visit_class_heritage(heritage);
    }
    escape.visit_class_body(&class.body);
  }

  /// Walk a variable declaration at the top level or inside a function. An initializer gets the
  /// demand of what its pattern reads.
  fn walk_var_decl(&mut self, decl: &'a VariableDeclaration<'a>) {
    // `using` calls a method of the value when the scope ends.
    let using =
      matches!(decl.kind, VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing);
    for declarator in &decl.declarations {
      if using {
        self.escape().visit_binding_pattern(&declarator.id);
        if let Some(init) = &declarator.init {
          self.walk_expr(init, &PathTree::whole());
        }
        continue;
      }
      self.an.register_local_decl(declarator, self.site);
      self.walk_pattern_parts(&declarator.id);
      if let Some(init) = &declarator.init {
        let demand = self.pattern_demand(self.module, &declarator.id);
        self.walk_expr(init, &demand);
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
    } else {
      self.an.state.add_entity_demand(key, demand);
    }
    for (index, prop) in obj.properties.iter().enumerate() {
      let p = match prop {
        ObjectPropertyKind::SpreadProperty(spread) => {
          self.walk_expr(&spread.argument, &PathTree::whole());
          continue;
        }
        ObjectPropertyKind::ObjectProperty(p) => p,
      };
      // The analysis does not model the calls of a getter or a setter.
      if p.kind != PropertyKind::Init
        && let Expression::FunctionExpression(func) = &p.value
      {
        self.an.state.escape_entity((self.module, func.node_id()));
      }
      if !is_proto_setter(p)
        && let Some(name) = self.an.property_key(p, self.site)
        && self.is_overwritten(obj, index, &name, escaped)
      {
        if self.catching || self.an.property_effects(p, self.removal_cx()) {
          self.walk_property(p, &PathTree::default());
        } else {
          self.excluded.push(Excluded::Prop(prop));
          self.rewrites.removed_props.insert(p.node_id());
        }
        continue;
      }
      if escaped {
        self.walk_property(p, &PathTree::whole());
        continue;
      }
      // A read of a key that the literal does not define reads the prototype.
      if is_proto_setter(p) {
        self.walk_property(p, demand);
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
      // A write runs a setter, and a write to a getter-only property throws. A read of each key
      // runs each getter.
      let needed = sub.is_some()
        || (p.kind != PropertyKind::Init && deopted)
        || (p.kind == PropertyKind::Get && demand.every_key);
      if needed {
        self.walk_property(p, &sub.unwrap_or_default());
      } else if self.catching || self.an.property_effects(p, self.removal_cx()) {
        self.walk_property(p, &PathTree::default());
      } else {
        self.excluded.push(Excluded::Prop(prop));
        self.rewrites.removed_props.insert(p.node_id());
      }
    }
  }

  /// Whether a later property of the literal replaces the property at `index`, so that no code
  /// can read its value. The key order of the object can still see it, because the first
  /// definition of a key decides the position of the key. So when unknown code can list the
  /// keys, the property goes only when the definitions of the same key follow it directly.
  fn is_overwritten(
    &mut self,
    obj: &'a ObjectExpression<'a>,
    index: usize,
    name: &str,
    escaped: bool,
  ) -> bool {
    let ObjectPropertyKind::ObjectProperty(p) = &obj.properties[index] else {
      return false;
    };
    for later in &obj.properties[index + 1..] {
      match later {
        ObjectPropertyKind::ObjectProperty(q)
          if !is_proto_setter(q) && self.an.property_key(q, self.site).as_deref() == Some(name) =>
        {
          // A getter and a setter of one key make one accessor property.
          if q.kind == PropertyKind::Init || p.kind == PropertyKind::Init || q.kind == p.kind {
            return true;
          }
        }
        _ if escaped => return false,
        _ => {}
      }
    }
    false
  }

  /// An element gets the demand at its index. After a spread, the indexes are unknown.
  fn walk_array(&mut self, arr: &'a ArrayExpression<'a>, demand: &PathTree) {
    let key = (self.module, arr.node_id());
    let escaped = demand.whole
      || self.an.state.all_objects_deopted
      || self.an.state.entity(key).is_some_and(|entity| entity.escaped);
    if escaped {
      self.an.state.escape_entity(key);
    } else {
      self.an.state.add_entity_demand(key, demand);
    }
    let mut after_spread = false;
    for (index, element) in arr.elements.iter().enumerate() {
      match element {
        ArrayExpressionElement::SpreadElement(spread) => {
          self.walk_expr(&spread.argument, &PathTree::whole());
          after_spread = true;
        }
        ArrayExpressionElement::Elision(_) => {}
        _ => {
          let sub = if escaped || after_spread {
            PathTree::whole()
          } else {
            demand.at(&index.to_string()).unwrap_or_default()
          };
          self.walk_expr(element.to_expression(), &sub);
        }
      }
    }
  }

  fn walk_property(&mut self, p: &'a ObjectProperty<'a>, demand: &PathTree) {
    if p.computed
      && let Some(key) = p.key.as_expression()
    {
      let key_demand = if is_primitive_key(&self.an.value(key, self.site)) {
        PathTree::default()
      } else {
        PathTree::whole()
      };
      self.walk_expr(key, &key_demand);
    }
    self.walk_expr(&p.value, demand);
  }

  pub fn walk_function(&mut self, func: FnNode<'a>) {
    let saved_site = self.site;
    let saved_func = self.func;
    self.site = saved_site.in_function();
    let value = FnValue { module: self.module, stmt: self.site.stmt, node: func, holder: None };
    self.an.register_params(&value);
    let catching = self.catching;
    self.catching |= self.an.state.caught_fns.contains(&value.key());
    self.func = Some(value);
    let params = func.params();
    for item in &params.items {
      self.walk_pattern_parts(&item.pattern);
      if let Some(init) = &item.initializer {
        let demand = self.pattern_demand(self.module, &item.pattern);
        self.walk_expr(init, &demand);
      }
    }
    if let Some(rest) = &params.rest {
      self.walk_pattern_parts(&rest.rest.argument);
    }
    match func.expression_body() {
      Some(expr) => {
        let demand = self.return_demand();
        self.walk_expr(expr, &demand);
      }
      None => self.walk_stmts(func.body_statements()),
    }
    self.catching = catching;
    self.func = saved_func;
    self.site = saved_site;
  }

  /// Walk a nested statement list, and remove the statements that do nothing. Declarations,
  /// `return`, and jumps always stay. After a statement that always jumps, the statements that
  /// are not declarations never run, so they go.
  fn walk_stmts(&mut self, stmts: &'a [Statement<'a>]) {
    self.an.register_local_fns(stmts, self.site);
    let mut reachable = true;
    for stmt in stmts {
      if !reachable && !stmt.is_declaration() && !collect::contains_var_decl(stmt) {
        self.exclude_stmt(stmt);
        self.rewrites.removed_stmts.insert(stmt.node_id());
        continue;
      }
      let removable = match stmt {
        Statement::ExpressionStatement(_) | Statement::EmptyStatement(_) => true,
        Statement::BlockStatement(_) | Statement::IfStatement(_) => {
          !collect::contains_var_decl(stmt)
        }
        _ => false,
      };
      if removable && !self.catching && !self.an.stmt_effects(stmt, self.removal_cx()) {
        self.exclude_stmt(stmt);
        self.rewrites.removed_stmts.insert(stmt.node_id());
        continue;
      }
      self.walk_stmt(stmt);
      reachable &= !self.always_jumps(stmt);
    }
  }

  /// Whether `stmt`, as the walk keeps it, ends with a jump on each path. A labeled statement
  /// can end normally through a `break` of its label.
  fn always_jumps(&self, stmt: &Statement<'a>) -> bool {
    match stmt {
      Statement::ReturnStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::BreakStatement(_)
      | Statement::ContinueStatement(_) => true,
      Statement::BlockStatement(block) => block.body.iter().any(|stmt| self.always_jumps(stmt)),
      Statement::IfStatement(if_stmt) => match self.rewrites.folded_ifs.get(&if_stmt.node_id()) {
        Some(folded) => {
          let live = if folded.keep_consequent {
            Some(&if_stmt.consequent)
          } else {
            if_stmt.alternate.as_ref()
          };
          live.is_some_and(|live| self.always_jumps(live))
        }
        None => {
          self.always_jumps(&if_stmt.consequent)
            && if_stmt.alternate.as_ref().is_some_and(|alternate| self.always_jumps(alternate))
        }
      },
      _ => false,
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
          let demand = self.return_demand();
          self.walk_expr(arg, &demand);
        }
      }
      Statement::ThrowStatement(throw) => self.walk_expr(&throw.argument, &PathTree::whole()),
      Statement::VariableDeclaration(decl) => self.walk_var_decl(decl),
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
        let catching = self.catching;
        self.catching |= collect::discards_throw(try_stmt);
        self.walk_stmts(&try_stmt.block.body);
        self.catching = catching;
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
        let keep_test = self.catching || self.an.expr_effects(&if_stmt.test, self.removal_cx());
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
      | Expression::StringLiteral(_)
      | Expression::ThisExpression(_)
      | Expression::Super(_)
      | Expression::ImportMeta(_)
      | Expression::NewTarget(_) => {}
      Expression::RegExpLiteral(re) => {
        let key = (self.module, re.node_id());
        if demand.whole {
          self.an.state.escape_entity(key);
        } else {
          self.an.state.add_entity_demand(key, demand);
        }
      }
      Expression::Identifier(ident) => self.an.use_ident(self.module, ident, demand),
      Expression::ParenthesizedExpression(paren) => self.walk_expr(&paren.expression, demand),
      oxc::ast::match_member_expression!(Expression) => {
        self.walk_member(expr.to_member_expression(), demand);
      }
      Expression::CallExpression(call) => self.walk_call(call, true, demand),
      Expression::NewExpression(new) => {
        self.walk_expr(&new.callee, &PathTree::whole());
        let callee = self.an.value(&new.callee, self.site);
        let ctor = self.an.ctor_args_fn(&callee);
        self.walk_arguments(ctor.as_ref(), &new.arguments);
      }
      Expression::ChainExpression(chain) => match &chain.expression {
        ChainElement::CallExpression(call) => self.walk_call(call, false, demand),
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
      Expression::ArrayExpression(arr) => self.walk_array(arr, demand),
      Expression::ClassExpression(class) => self.walk_class(class, demand),
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
      Expression::FunctionExpression(func) => {
        self.on_fn_value(FnNode::Function(func), demand);
        self.walk_function(FnNode::Function(func));
      }
      Expression::ArrowFunctionExpression(arrow) => {
        self.on_fn_value(FnNode::Arrow(arrow), demand);
        self.walk_function(FnNode::Arrow(arrow));
      }
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

  /// A function value with a demand that lets unknown code call it escapes.
  fn on_fn_value(&mut self, func: FnNode<'a>, demand: &PathTree) {
    if Analyzer::is_call_revealing(demand) {
      self.an.state.escape_entity((self.module, func.node_id()));
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

  /// Walk a call whose value the context uses at the paths in `demand`. A known callee needs
  /// only the property that holds it, unless it reads `this`, and the call is a recorded call of
  /// it. An unknown callee escapes, because the analysis cannot record its calls. An argument of
  /// a known callee gets the demand of its parameter, and it goes when the callee never reads
  /// the parameter and the argument does nothing.
  fn walk_call(&mut self, call: &'a CallExpression<'a>, can_remove_args: bool, demand: &PathTree) {
    let callee_fn = match self.an.value(&call.callee, self.site) {
      Value::Function(func) => Some(func),
      _ => None,
    };
    if let Some(func) = &callee_fn {
      self.an.record_call(func, call, self.site);
      self.an.state.add_return_demand(func.key(), demand);
    }
    if !call.optional
      && let Expression::Identifier(ident) = call.callee.without_parentheses()
      && ident.name == "eval"
      && matches!(self.an.resolve(self.module, ident), Resolved::Global)
    {
      self.an.on_direct_eval(self.module);
    }
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
      _ => {
        let callee_demand =
          if callee_fn.is_some() { PathTree::default() } else { PathTree::whole() };
        self.walk_expr(&call.callee, &callee_demand);
      }
    }
    let has_spread = call.arguments.iter().any(|arg| matches!(arg, Argument::SpreadElement(_)));
    let tracked = callee_fn.filter(|func| !self.an.fn_uses_arguments(func));
    let removable = tracked.filter(|_| can_remove_args && !has_spread);
    let mut after_spread = false;
    for (index, arg) in call.arguments.iter().enumerate() {
      let Some(expr) = arg.as_expression() else {
        if let Argument::SpreadElement(spread) = arg {
          let caught = callee_fn.as_ref().is_some_and(|func| self.an.is_arg_caught(func, None));
          self.walk_arg(&spread.argument, caught, &PathTree::whole());
        }
        after_spread = true;
        continue;
      };
      let position = (!after_spread).then_some(index);
      let caught = callee_fn.as_ref().is_some_and(|func| self.an.is_arg_caught(func, position));
      if let Some(func) = &removable
        && !caught
        && self.is_param_unused(func, index)
        && !self.catching
        && !self.an.expr_effects(expr, self.removal_cx())
      {
        self.excluded.push(Excluded::Arg(arg));
        self.rewrites.removed_args.insert(arg.node_id());
        continue;
      }
      let arg_demand = match &tracked {
        Some(func) if !after_spread => self.param_demand(func, index),
        _ => PathTree::whole(),
      };
      self.walk_arg(expr, caught, &arg_demand);
    }
  }

  /// Walk the arguments of a `new`. `callee` is the function that gets them.
  fn walk_arguments(&mut self, callee: Option<&FnValue<'a>>, arguments: &'a [Argument<'a>]) {
    let mut after_spread = false;
    for (index, arg) in arguments.iter().enumerate() {
      after_spread |= matches!(arg, Argument::SpreadElement(_));
      let position = (!after_spread).then_some(index);
      let caught = callee.is_some_and(|func| self.an.is_arg_caught(func, position));
      let expr = match arg {
        Argument::SpreadElement(spread) => &spread.argument,
        _ => arg.to_expression(),
      };
      self.walk_arg(expr, caught, &PathTree::whole());
    }
  }

  /// Walk an argument. When a catching `try` block in the callee can call the argument
  /// (`caught`), the code after that block sees each throw of it. Then the walk keeps the code of
  /// the argument that runs, like the code of that block, and a function that the argument gives
  /// keeps the code of its body that runs. Rollup's default `treeshake.tryCatchDeoptimization`
  /// does the same for a parameter that a `try` block calls. See
  /// internal-docs/advanced-tree-shaking/design.md.
  fn walk_arg(&mut self, expr: &'a Expression<'a>, caught: bool, demand: &PathTree) {
    if !caught {
      self.walk_expr(expr, demand);
      return;
    }
    if let Value::Function(func) = self.an.value(expr, self.site) {
      self.an.state.add_caught_fn(func.key());
    }
    let catching = self.catching;
    self.catching = true;
    self.walk_expr(expr, demand);
    self.catching = catching;
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
        && !self.catching
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
      Some(false) if !self.catching && !self.an.expr_effects(&logical.left, self.removal_cx()) => {
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
      && !self.catching
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
        let member = target.to_member_expression();
        self.an.deopt_member_delete(member, self.site);
        self.walk_member_write(member, false);
      }
      arg => self.walk_expr(arg, &PathTree::default()),
    }
  }
}
