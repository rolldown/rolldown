//! Syntactic helpers of the analysis: the reference collector for excluded code, and checks that
//! only look at the AST.

use oxc::{
  ast::{
    AstKind,
    ast::{
      Argument, Class, Expression, Function, ObjectExpression, ObjectPropertyKind, Statement,
      VariableDeclarationKind,
    },
  },
  ast_visit::{VisitJs, walk_js},
  semantic::{NodeId, ReferenceId, ScopeFlags, SymbolId},
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
  let mut finder = Finder {
    matches: |kind: &AstKind| matches!(kind, AstKind::ThisExpression(_) | AstKind::Super(_)),
    into_arrows: true,
    into_classes: true,
    found: false,
  };
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
