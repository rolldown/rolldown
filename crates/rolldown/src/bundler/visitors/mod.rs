pub mod scanner;

use std::cell::Cell;

use oxc::{
  allocator::{Allocator, Box, Vec},
  ast::{
    ast::{
      BindingIdentifier, BindingPattern, Declaration, Statement, VariableDeclaration,
      VariableDeclarator,
    },
    VisitMut,
  },
  semantic::{ScopeTree, SymbolId},
  span::Atom,
};
use rolldown_common::{ModuleId, SymbolRef};
use rolldown_oxc::{BindingIdentifierExt, TakeIn};
use rustc_hash::FxHashMap;

use super::graph::symbols::Symbols;

pub struct FinalizeContext<'ast> {
  pub allocator: &'ast Allocator,
  pub symbols: &'ast Symbols,
  pub scope: &'ast ScopeTree,
  pub final_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub default_export_symbol: Option<SymbolId>,
  pub id: ModuleId,
}

pub struct Finalizer<'ast> {
  ctx: FinalizeContext<'ast>,
}

impl<'ast> Finalizer<'ast> {
  pub fn new(ctx: FinalizeContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn should_keep_this_top_level_stmt(&self, stmt: &Statement) -> bool {
    const REMOVE_THIS_STMT: bool = false;
    const KEEP_THIS_STMT: bool = true;

    match stmt {
      Statement::ModuleDeclaration(decl) => match &decl.0 {
        oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(decl)
          if decl.declaration.is_some() =>
        {
          KEEP_THIS_STMT
        }
        // FIXME: ugly hack
        oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(decl) if decl.span.size() != 0 => {
          REMOVE_THIS_STMT
        }
        oxc::ast::ast::ModuleDeclaration::ImportDeclaration(_)
        | oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(_) => REMOVE_THIS_STMT,
        _ => KEEP_THIS_STMT,
      },
      _ => KEEP_THIS_STMT,
    }
  }
}

impl<'ast, 'p> VisitMut<'ast, 'p> for Finalizer<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'p mut BindingIdentifier) {
    let symbol_ref = (self.ctx.id, ident.expect_symbol_id()).into();
    let final_ref = self.ctx.symbols.par_get_canonical_ref(symbol_ref);
    if let Some(name) = self.ctx.final_names.get(&final_ref) {
      if ident.name != name {
        ident.name = name.clone()
      }
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'p mut oxc::ast::ast::IdentifierReference) {
    if let Some(symbol_id) =
      self.ctx.symbols.tables[self.ctx.id].references[ident.reference_id.get().unwrap()]
    {
      let symbol_ref = (self.ctx.id, symbol_id).into();
      let final_ref = self.ctx.symbols.par_get_canonical_ref(symbol_ref);
      if let Some(name) = self.ctx.final_names.get(&final_ref) {
        if ident.name != name {
          ident.name = name.clone()
        }
      }
    }
  }

  fn visit_program(&mut self, program: &'p mut oxc::ast::ast::Program<'ast>) {
    let alloc = self.ctx.allocator;
    program
      .body
      .retain(|stmt| self.should_keep_this_top_level_stmt(stmt));
    program.body.iter_mut().for_each(|stmt| {
      if let Statement::ModuleDeclaration(decl) = stmt {
        match decl.0 {
          oxc::ast::ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
            match &mut decl.declaration {
              oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
                let mut declarations = Vec::new_in(self.ctx.allocator);
                declarations.push(VariableDeclarator {
                  span: Default::default(),
                  kind: oxc::ast::ast::VariableDeclarationKind::Var,
                  id: BindingPattern {
                    kind: oxc::ast::ast::BindingPatternKind::BindingIdentifier(Box(
                      self.ctx.allocator.alloc(BindingIdentifier {
                        span: Default::default(),
                        name: "".into(),
                        symbol_id: Cell::new(self.ctx.default_export_symbol),
                      }),
                    )),
                    type_annotation: None,
                    optional: false,
                  },
                  init: Some(exp.take_in(self.ctx.allocator)),
                  definite: false,
                });
                *stmt = Statement::Declaration(Declaration::VariableDeclaration(Box(alloc.alloc(
                  VariableDeclaration {
                    span: Default::default(),
                    kind: oxc::ast::ast::VariableDeclarationKind::Var,
                    declarations,
                    modifiers: Default::default(),
                  },
                ))))
              }
              oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
                *stmt = Statement::Declaration(oxc::ast::ast::Declaration::FunctionDeclaration(
                  decl.take_in(alloc),
                ))
              }
              oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
                *stmt = Statement::Declaration(oxc::ast::ast::Declaration::ClassDeclaration(
                  decl.take_in(alloc),
                ))
              }
              _ => {}
            }
          }
          oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(named_decl) => {
            if let Some(decl) = &mut named_decl.declaration {
              *stmt = Statement::Declaration(decl.take_in(alloc))
            }
          }
          _ => {}
        }
      }
    });

    self.visit_statements(&mut program.body);
  }
}
