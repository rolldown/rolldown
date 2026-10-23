//! Syntactic helpers of the analysis: the reference collector for excluded code, and checks that
//! only look at the AST.

use oxc::{
  ast::{
    AstKind,
    ast::{
      Argument, Class, ClassElement, Expression, FormalParameters, Function, ObjectExpression,
      ObjectPropertyKind, Program, PropertyKind, Statement, SwitchCase, TryStatement,
      VariableDeclarationKind,
    },
  },
  ast_visit::{VisitJs, walk_js},
  semantic::{NodeId, ReferenceId, ScopeFlags, ScopeId, Scoping, SymbolFlags, SymbolId},
};
use rolldown_common::{AstScopes, ImportRecordIdx};
use rustc_hash::{FxHashMap, FxHashSet};

/// A part of an included statement that the output does not contain.
#[derive(Clone, Copy)]
pub enum Excluded<'a> {
  Stmt(&'a Statement<'a>),
  Expr(&'a Expression<'a>),
  Prop(&'a ObjectPropertyKind<'a>),
  Arg(&'a Argument<'a>),
}

/// The references in a piece of code.
#[derive(Default)]
pub struct Refs {
  /// How many identifier references resolve to each symbol.
  pub symbols: FxHashMap<SymbolId, u32>,
  /// Node ids of the member expressions.
  pub members: FxHashSet<NodeId>,
  /// Node ids that `module.imports` maps to an import record.
  pub imports: FxHashSet<NodeId>,
}

pub struct RefCollector<'r> {
  pub scopes: &'r AstScopes,
  pub imports: &'r FxHashMap<NodeId, ImportRecordIdx>,
  /// The object identifiers of the statement's `MemberExpr` entries. They are not `Symbol`
  /// entries, so they do not count.
  pub member_roots: &'r FxHashSet<ReferenceId>,
  pub refs: Refs,
}

impl RefCollector<'_> {
  pub fn collect(&mut self, part: Excluded) {
    match part {
      Excluded::Stmt(stmt) => self.visit_statement(stmt),
      Excluded::Expr(expr) => self.visit_expression(expr),
      Excluded::Prop(prop) => self.visit_object_property_kind(prop),
      Excluded::Arg(arg) => self.visit_argument(arg),
    }
  }
}

impl<'v> VisitJs<'v> for RefCollector<'_> {
  fn enter_node(&mut self, kind: AstKind<'v>) {
    match kind {
      AstKind::IdentifierReference(ident) => {
        if let Some(ref_id) = ident.reference_id.get()
          && !self.member_roots.contains(&ref_id)
          && let Some(symbol_id) = self.scopes.symbol_id_for(ref_id)
        {
          *self.refs.symbols.entry(symbol_id).or_default() += 1;
        }
      }
      AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_) => {
        self.refs.members.insert(kind.node_id());
      }
      _ => {}
    }
    if !self.imports.is_empty() {
      let node_id = kind.node_id();
      if self.imports.contains_key(&node_id) {
        self.refs.imports.insert(node_id);
      }
    }
  }
}

/// Finds a node kind in code that runs in the current function, so it skips nested functions
/// (arrow functions too, unless `into_arrows`). In a class it skips everything, unless
/// `into_classes`: then it visits all parts of the class except the methods. The `extends`
/// clause and the computed keys run in the current function, and the other parts only make the
/// search find more.
struct Finder<F> {
  matches: F,
  into_arrows: bool,
  into_classes: bool,
  found: bool,
}

impl<'v, F: FnMut(&AstKind<'v>) -> bool> VisitJs<'v> for Finder<F> {
  fn enter_node(&mut self, kind: AstKind<'v>) {
    if !self.found && (self.matches)(&kind) {
      self.found = true;
    }
  }

  fn visit_function(&mut self, _it: &Function<'v>, _flags: ScopeFlags) {}

  fn visit_arrow_function_expression(&mut self, it: &oxc::ast::ast::ArrowFunctionExpression<'v>) {
    if self.into_arrows {
      walk_js::walk_arrow_function_expression(self, it);
    }
  }

  fn visit_class(&mut self, it: &Class<'v>) {
    if self.into_classes {
      walk_js::walk_class(self, it);
    }
  }
}

/// Whether the body of `func` reads `this` or `super`. Arrow functions inside see the same
/// values.
pub fn function_uses_this(func: &Function) -> bool {
  let mut finder = this_finder();
  walk_function_parts(&mut finder, func);
  finder.found
}

/// Whether the body of `func` reads its `arguments` object.
pub fn function_uses_arguments(func: &Function) -> bool {
  let mut finder = Finder {
    matches: |kind: &AstKind| matches!(kind, AstKind::IdentifierReference(ident) if ident.name == "arguments"),
    into_arrows: true,
    into_classes: true,
    found: false,
  };
  walk_function_parts(&mut finder, func);
  finder.found
}

fn walk_function_parts<'v, V: VisitJs<'v>>(visitor: &mut V, func: &Function<'v>) {
  visitor.visit_formal_parameters(&func.params);
  if let Some(body) = &func.body {
    visitor.visit_function_body(body);
  }
}

fn this_finder() -> Finder<impl FnMut(&AstKind) -> bool> {
  Finder {
    matches: |kind: &AstKind| matches!(kind, AstKind::ThisExpression(_) | AstKind::Super(_)),
    into_arrows: true,
    into_classes: true,
    found: false,
  }
}

/// Whether code of the class can reach the class object in a way that the analysis does not
/// track: a static element reads `this` or `super`, an `accessor` property exists, or a class
/// expression refers to its own name. The name of a class declaration is a top-level binding,
/// so the walk of the class sees each reference to it.
pub fn class_opaque(class: &Class) -> bool {
  let static_this = class.body.body.iter().any(|element| match element {
    ClassElement::MethodDefinition(method) => method.r#static && function_uses_this(&method.value),
    ClassElement::PropertyDefinition(prop) => {
      prop.r#static
        && prop.value.as_ref().is_some_and(|value| {
          let mut finder = this_finder();
          finder.visit_expression(value);
          finder.found
        })
    }
    ClassElement::StaticBlock(block) => {
      let mut finder = this_finder();
      finder.visit_statements(&block.body);
      finder.found
    }
    ClassElement::AccessorProperty(_) => true,
    ClassElement::TSIndexSignature(_) => false,
  });
  static_this
    || (class.is_expression()
      && class.id.as_ref().is_some_and(|id| {
        let mut finder = NameFinder { name: id.name.as_str(), found: false };
        finder.visit_class(class);
        finder.found
      }))
}

/// Finds a reference to a name anywhere, nested functions included.
struct NameFinder<'n> {
  name: &'n str,
  found: bool,
}

impl<'v> VisitJs<'v> for NameFinder<'_> {
  fn enter_node(&mut self, kind: AstKind<'v>) {
    if let AstKind::IdentifierReference(ident) = kind
      && ident.name == self.name
    {
      self.found = true;
    }
  }
}

/// Whether a method of the object literal reads `this` or `super`.
pub fn object_uses_this(obj: &ObjectExpression) -> bool {
  obj.properties.iter().any(|prop| match prop {
    ObjectPropertyKind::ObjectProperty(prop) => {
      matches!(&prop.value, Expression::FunctionExpression(func) if function_uses_this(func))
    }
    ObjectPropertyKind::SpreadProperty(_) => false,
  })
}

/// The arguments of the `return` statements of the current function.
pub fn return_arguments<'a>(stmts: &'a [Statement<'a>], out: &mut Vec<&'a Expression<'a>>) {
  for stmt in stmts {
    return_arguments_in(stmt, out);
  }
}

fn return_arguments_in<'a>(stmt: &'a Statement<'a>, out: &mut Vec<&'a Expression<'a>>) {
  match stmt {
    Statement::ReturnStatement(ret) => out.extend(ret.argument.as_ref()),
    Statement::BlockStatement(block) => return_arguments(&block.body, out),
    Statement::IfStatement(if_stmt) => {
      return_arguments_in(&if_stmt.consequent, out);
      if let Some(alternate) = &if_stmt.alternate {
        return_arguments_in(alternate, out);
      }
    }
    Statement::ForStatement(for_stmt) => return_arguments_in(&for_stmt.body, out),
    Statement::ForInStatement(for_in) => return_arguments_in(&for_in.body, out),
    Statement::ForOfStatement(for_of) => return_arguments_in(&for_of.body, out),
    Statement::WhileStatement(while_stmt) => return_arguments_in(&while_stmt.body, out),
    Statement::DoWhileStatement(do_while) => return_arguments_in(&do_while.body, out),
    Statement::SwitchStatement(switch) => {
      for case in &switch.cases {
        return_arguments(&case.consequent, out);
      }
    }
    Statement::TryStatement(try_stmt) => {
      return_arguments(&try_stmt.block.body, out);
      if let Some(handler) = &try_stmt.handler {
        return_arguments(&handler.body.body, out);
      }
      if let Some(finalizer) = &try_stmt.finalizer {
        return_arguments(&finalizer.body, out);
      }
    }
    Statement::LabeledStatement(labeled) => return_arguments_in(&labeled.body, out),
    Statement::WithStatement(with) => return_arguments_in(&with.body, out),
    _ => {}
  }
}

/// Whether `stmt` contains a `return` of the current function.
pub fn contains_return(stmt: &Statement) -> bool {
  let mut finder = Finder {
    matches: |kind: &AstKind| matches!(kind, AstKind::ReturnStatement(_)),
    into_arrows: false,
    into_classes: false,
    found: false,
  };
  finder.visit_statement(stmt);
  finder.found
}

/// Whether `stmt` declares a `var` of the current function. Removing such a statement would
/// remove the declaration.
pub fn contains_var_decl(stmt: &Statement) -> bool {
  let mut finder = Finder {
    matches: |kind: &AstKind| matches!(kind, AstKind::VariableDeclaration(decl) if decl.kind == VariableDeclarationKind::Var),
    into_arrows: false,
    into_classes: false,
    found: false,
  };
  finder.visit_statement(stmt);
  finder.found
}

/// Whether `try_stmt` can discard a throw of its block: it has a `catch` clause, or its `finally`
/// block has a `return`, a `break`, or a `continue`. Its block is a catching `try` block. The code
/// that runs after such a block sees whether the block throws, also when code that oxc treats as
/// pure throws. See internal-docs/advanced-tree-shaking/design.md.
pub fn discards_throw(try_stmt: &TryStatement) -> bool {
  try_stmt.handler.is_some()
    || try_stmt.finalizer.as_ref().is_some_and(|finalizer| {
      let mut finder = Finder {
        matches: |kind: &AstKind| {
          matches!(
            kind,
            AstKind::ReturnStatement(_)
              | AstKind::BreakStatement(_)
              | AstKind::ContinueStatement(_)
          )
        },
        into_arrows: false,
        into_classes: false,
        found: false,
      };
      finder.visit_block_statement(finalizer);
      finder.found
    })
}

/// The parameters of a function whose arguments a catching `try` block (see [`discards_throw`])
/// in the function can call. Such a block reads the parameter, or the `arguments` object of the
/// function. The check compares names, so another binding with the same name only makes the walk
/// keep more code. See internal-docs/advanced-tree-shaking/design.md.
#[derive(Debug, Default)]
pub struct CaughtParams {
  /// The positions of the parameters. The position of the rest parameter is the number of the
  /// other parameters.
  positions: Vec<usize>,
  /// Such a block reads `arguments`, which gives each argument.
  all: bool,
}

impl CaughtParams {
  /// Whether such a block can call the argument at `index`. `None` is an argument at an unknown
  /// position, after a spread argument.
  pub fn contains(&self, params: &FormalParameters, index: Option<usize>) -> bool {
    if self.all {
      return true;
    }
    match index {
      None => !self.positions.is_empty(),
      Some(index) if index < params.items.len() => self.positions.contains(&index),
      Some(_) => params.rest.is_some() && self.positions.contains(&params.items.len()),
    }
  }
}

/// The parameters of a function whose arguments a catching `try` block in the function can call.
/// Nested functions count, because the block can call them. `has_arguments` is for a function that
/// has its own `arguments` object.
pub fn caught_params(
  params: &FormalParameters,
  body: &[Statement],
  has_arguments: bool,
) -> CaughtParams {
  struct CaughtParamFinder<'n> {
    names: &'n [(&'n str, usize)],
    has_arguments: bool,
    /// The number of catching `try` blocks around the visit.
    caught: u32,
    found: CaughtParams,
  }

  impl<'v> VisitJs<'v> for CaughtParamFinder<'_> {
    fn enter_node(&mut self, kind: AstKind<'v>) {
      let AstKind::IdentifierReference(ident) = kind else {
        return;
      };
      if self.caught == 0 {
        return;
      }
      if self.has_arguments && ident.name == "arguments" {
        self.found.all = true;
      }
      for &(name, position) in self.names {
        if ident.name == name && !self.found.positions.contains(&position) {
          self.found.positions.push(position);
        }
      }
    }

    fn visit_try_statement(&mut self, it: &TryStatement<'v>) {
      let caught = u32::from(discards_throw(it));
      self.caught += caught;
      self.visit_block_statement(&it.block);
      self.caught -= caught;
      if let Some(handler) = &it.handler {
        self.visit_catch_clause(handler);
      }
      if let Some(finalizer) = &it.finalizer {
        self.visit_block_statement(finalizer);
      }
    }
  }

  let mut names = Vec::new();
  for (position, item) in params.items.iter().enumerate() {
    for id in item.pattern.get_binding_identifiers() {
      names.push((id.name.as_str(), position));
    }
  }
  if let Some(rest) = &params.rest {
    for id in rest.rest.argument.get_binding_identifiers() {
      names.push((id.name.as_str(), params.items.len()));
    }
  }
  let mut finder =
    CaughtParamFinder { names: &names, has_arguments, caught: 0, found: CaughtParams::default() };
  finder.visit_formal_parameters(params);
  for stmt in body {
    finder.visit_statement(stmt);
  }
  finder.found
}

/// Whether `new` of the class puts elements on the new object: instance fields, `accessor`
/// properties, or private methods.
pub fn installs_instance_elements(class: &Class) -> bool {
  class.body.body.iter().any(|element| match element {
    ClassElement::PropertyDefinition(prop) => !prop.r#static,
    ClassElement::AccessorProperty(prop) => !prop.r#static,
    ClassElement::MethodDefinition(method) => {
      !method.r#static && method.key.is_private_identifier()
    }
    ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => false,
  })
}

/// The functions of a module that are methods, getters, setters, or constructors of an object
/// literal or a class. `new` of such a function throws.
pub fn method_functions(program: &Program) -> FxHashSet<NodeId> {
  struct Methods(FxHashSet<NodeId>);

  impl<'v> VisitJs<'v> for Methods {
    fn enter_node(&mut self, kind: AstKind<'v>) {
      match kind {
        AstKind::ObjectProperty(prop) if prop.method || prop.kind != PropertyKind::Init => {
          if let Expression::FunctionExpression(func) = &prop.value {
            self.0.insert(func.node_id());
          }
        }
        AstKind::MethodDefinition(method) => {
          self.0.insert(method.value.node_id());
        }
        _ => {}
      }
    }
  }

  let mut methods = Methods(FxHashSet::default());
  methods.visit_program(program);
  methods.0
}

/// Facts about the local `let` and `const` bindings of one top-level statement. They decide
/// whether a read of such a binding can run before the initialization, in the temporal dead
/// zone. See internal-docs/advanced-tree-shaking/implementation.md.
#[derive(Default)]
pub struct LocalTdz {
  /// The end of the declarator of each `let` or `const` with an initializer. A declaration in a
  /// `case` clause is not here: a jump to a later clause skips it.
  decl_end: FxHashMap<SymbolId, u32>,
  /// The start of each reference to a local `let`, `const`, or function declaration.
  ref_start: FxHashMap<ReferenceId, u32>,
  /// The start of each function, keyed by its scope, with the binding of a function
  /// declaration. A function declaration exists from the start of the scope that contains it.
  fn_scopes: FxHashMap<ScopeId, (u32, Option<SymbolId>)>,
}

impl LocalTdz {
  pub fn collect(stmt: &Statement, scoping: &Scoping) -> Self {
    let mut collector = LocalTdzCollector { scoping, tdz: Self::default(), case_decls: Vec::new() };
    collector.visit_statement(stmt);
    let LocalTdzCollector { mut tdz, case_decls, .. } = collector;
    for symbol_id in case_decls {
      tdz.decl_end.remove(&symbol_id);
    }
    tdz
  }

  /// Whether the reference `ref_id` to the local binding `symbol_id` runs only after the
  /// initialization of the binding.
  pub fn is_read_safe(&self, scoping: &Scoping, symbol_id: SymbolId, ref_id: ReferenceId) -> bool {
    let (Some(&end), Some(&start)) = (self.decl_end.get(&symbol_id), self.ref_start.get(&ref_id))
    else {
      return false;
    };
    let decl_scope = scoping.symbol_scope_id(symbol_id);
    let scope = scoping.get_reference(ref_id).scope_id();
    self.runs_after(scoping, decl_scope, end, start, scope, &mut FxHashSet::default())
  }

  /// Whether code at `start` in `scope`, a scope inside `decl_scope`, runs only after the
  /// position `end` in the same run of `decl_scope`. In one run of a scope, the statements run
  /// in source order, apart from the jumps of a `switch`. A function runs only after its
  /// creation. `checked` holds the function declarations whose check is in progress or passed.
  fn runs_after(
    &self,
    scoping: &Scoping,
    decl_scope: ScopeId,
    end: u32,
    start: u32,
    scope: ScopeId,
    checked: &mut FxHashSet<SymbolId>,
  ) -> bool {
    // The outermost function between the code and the declaration.
    let mut outermost = None;
    let mut current = scope;
    while current != decl_scope {
      let flags = scoping.scope_flags(current);
      if flags.is_with() {
        return false;
      }
      if flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow | ScopeFlags::ClassStaticBlock) {
        outermost = Some(current);
      }
      let Some(parent) = scoping.scope_parent_id(current) else {
        return false;
      };
      current = parent;
    }
    let Some(func_scope) = outermost else {
      return start >= end;
    };
    let Some(&(func_start, declared)) = self.fn_scopes.get(&func_scope) else {
      return false;
    };
    let Some(binding) = declared else {
      return func_start >= end;
    };
    // The declaration can run early only through a reference that runs early. A reference in
    // the function itself runs only after a first call from outside.
    if !checked.insert(binding) {
      return true;
    }
    scoping.get_resolved_reference_ids(binding).iter().all(|&ref_id| {
      let ref_scope = scoping.get_reference(ref_id).scope_id();
      is_scope_inside(scoping, ref_scope, func_scope)
        || self.ref_start.get(&ref_id).is_some_and(|&ref_start| {
          self.runs_after(scoping, decl_scope, end, ref_start, ref_scope, checked)
        })
    })
  }
}

fn is_scope_inside(scoping: &Scoping, mut scope: ScopeId, ancestor: ScopeId) -> bool {
  loop {
    if scope == ancestor {
      return true;
    }
    match scoping.scope_parent_id(scope) {
      Some(parent) => scope = parent,
      None => return false,
    }
  }
}

struct LocalTdzCollector<'s> {
  scoping: &'s Scoping,
  tdz: LocalTdz,
  /// The bindings that a declaration directly in a `case` clause declares.
  case_decls: Vec<SymbolId>,
}

impl<'v> VisitJs<'v> for LocalTdzCollector<'_> {
  fn enter_node(&mut self, kind: AstKind<'v>) {
    match kind {
      AstKind::VariableDeclaration(decl)
        if matches!(decl.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Const) =>
      {
        for declarator in &decl.declarations {
          if declarator.init.is_none() {
            continue;
          }
          for id in declarator.id.get_binding_identifiers() {
            if let Some(symbol_id) = id.symbol_id.get() {
              self.tdz.decl_end.insert(symbol_id, declarator.span.end);
            }
          }
        }
      }
      AstKind::IdentifierReference(ident) => {
        if let Some(ref_id) = ident.reference_id.get()
          && let Some(symbol_id) = self.scoping.get_reference(ref_id).symbol_id()
          && self.scoping.symbol_scope_id(symbol_id) != self.scoping.root_scope_id()
          && self
            .scoping
            .symbol_flags(symbol_id)
            .intersects(SymbolFlags::BlockScopedVariable | SymbolFlags::Function)
        {
          self.tdz.ref_start.insert(ref_id, ident.span.start);
        }
      }
      AstKind::Function(func) => {
        if let Some(scope_id) = func.scope_id.get() {
          let declared = if func.is_function_declaration() {
            func.id.as_ref().and_then(|id| id.symbol_id.get())
          } else {
            None
          };
          self.tdz.fn_scopes.insert(scope_id, (func.span.start, declared));
        }
      }
      AstKind::ArrowFunctionExpression(arrow) => {
        if let Some(scope_id) = arrow.scope_id.get() {
          self.tdz.fn_scopes.insert(scope_id, (arrow.span.start, None));
        }
      }
      AstKind::StaticBlock(block) => {
        if let Some(scope_id) = block.scope_id.get() {
          self.tdz.fn_scopes.insert(scope_id, (block.span.start, None));
        }
      }
      _ => {}
    }
  }

  fn visit_switch_case(&mut self, case: &SwitchCase<'v>) {
    for stmt in &case.consequent {
      if let Statement::VariableDeclaration(decl) = stmt {
        self.case_decls.extend(
          decl
            .declarations
            .iter()
            .flat_map(|declarator| declarator.id.get_binding_identifiers())
            .filter_map(|id| id.symbol_id.get()),
        );
      }
    }
    walk_js::walk_switch_case(self, case);
  }
}
