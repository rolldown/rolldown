use oxc::{
  allocator::Allocator,
  ast::{ast, VisitMut},
  span::Atom,
};
use rolldown_common::{ModuleId, SymbolRef};
use rolldown_oxc::{AstSnippet, BindingIdentifierExt, IntoIn};
use rustc_hash::FxHashMap;

use super::{ast_scope::AstScope, symbols::Symbols};

pub struct FinalizerContext<'me> {
  pub id: ModuleId,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Atom>,
}

pub struct Finalizer<'me> {
  pub alloc: &'me Allocator,
  pub ctx: FinalizerContext<'me>,
  pub scope: &'me AstScope,
  pub snippet: &'me AstSnippet<'me>,
}

impl<'me> Finalizer<'me> {
  pub fn canonical_name_for(&self, symbol: SymbolRef) -> Option<&'me Atom> {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  // pub fn canonical_name_for_runtime(&self, name: &str) -> &Atom {
  //   let symbol = self.ctx.graph.runtime.resolve_symbol(&Atom::new_inline(name));
  //   self.canonical_name_for(symbol)
  // }
}
// visit

impl<'ast, 'me: 'ast> Finalizer<'me> {
  fn visit_top_level_statement_mut(&mut self, stmt: &mut ast::Statement<'ast>) {
    // FIXME: this is a hack to avoid renaming import statements
    if let ast::Statement::ModuleDeclaration(module_decl) = stmt {
      if matches!(module_decl.0, ast::ModuleDeclaration::ImportDeclaration(_)) {
        return;
      }
    }

    self.visit_statement(stmt);
  }
}

impl<'ast, 'me: 'ast> VisitMut<'ast> for Finalizer<'me> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    for directive in program.directives.iter_mut() {
      self.visit_directive(directive);
    }
    for stmt in program.body.iter_mut() {
      self.visit_top_level_statement_mut(stmt);
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier) {
    let symbol_ref: SymbolRef = (self.ctx.id, ident.expect_symbol_id()).into();

    let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbols.get(canonical_ref);
    assert!(symbol.namespace_alias.is_none());
    if let Some(canonical_name) = self.canonical_name_for(symbol_ref) {
      if ident.name != canonical_name {
        ident.name = canonical_name.clone();
      }
    } else {
      // FIXME: all bindings should have a canonical name
    }
  }

  #[allow(clippy::collapsible_else_if)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let ast::Expression::Identifier(id_ref) = expr {
      if let Some(symbol_id) = self.scope.symbol_id_for(id_ref.reference_id.get().unwrap()) {
        let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
        let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
        let symbol = self.ctx.symbols.get(canonical_ref);

        if let Some(ns_alias) = &symbol.namespace_alias {
          let canonical_ns_name = self
            .canonical_name_for(ns_alias.namespace_ref)
            .expect("namespace alias should have a canonical name");
          let prop_name = &ns_alias.property_name;
          *expr = ast::Expression::MemberExpression(
            self
              .snippet
              .identifier_member_expression(canonical_ns_name.clone(), prop_name.clone())
              .into_in(self.alloc),
          );
        } else {
          if let Some(canonical_name) = self.canonical_name_for(canonical_ref) {
            if id_ref.name != canonical_name {
              id_ref.name = canonical_name.clone();
            }
          } else {
            // FIXME: all bindings should have a canonical name
          }
        }
      };
    } else {
      self.visit_expression_match(expr);
    }
  }
}
