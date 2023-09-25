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
use rolldown_common::{ModuleId, ResolvedExport, SymbolRef};
use rolldown_oxc::{BindingIdentifierExt, TakeIn};
use rustc_hash::{FxHashMap, FxHashSet};

use super::graph::symbols::Symbols;

pub struct FinalizeContext<'ast> {
  pub allocator: &'ast Allocator,
  pub symbols: &'ast Symbols,
  pub scope: &'ast ScopeTree,
  pub final_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub default_export_symbol: Option<SymbolId>,
  pub id: ModuleId,
  pub external_requests: &'ast FxHashSet<Atom>,
  pub resolved_exports: &'ast FxHashMap<Atom, ResolvedExport>,
}

pub struct Finalizer<'ast> {
  ctx: FinalizeContext<'ast>,
}

impl<'ast> Finalizer<'ast> {
  pub fn new(ctx: FinalizeContext<'ast>) -> Self {
    Self { ctx }
  }

  #[inline]
  pub fn is_external_request(&self, request: &Atom) -> bool {
    self.ctx.external_requests.contains(request)
  }

  pub fn namespace_export_decl_to_namespace_import_decl_stmt(
    &self,
    decl: &oxc::ast::ast::ExportAllDeclaration,
  ) -> oxc::ast::ast::Statement<'ast> {
    if let Some(oxc::ast::ast::ModuleExportName::Identifier(ref exported)) = &decl.exported {
      let import_decl = self.ctx.allocator.alloc(oxc::ast::ast::ImportDeclaration {
        span: Default::default(),
        source: decl.source.clone(),
        assertions: None,
        import_kind: decl.export_kind,
        specifiers: Vec::from_iter_in(
          vec![
            oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
              oxc::ast::ast::ImportNamespaceSpecifier {
                span: Default::default(),
                local: BindingIdentifier {
                  span: Default::default(),
                  name: exported.name.clone(),
                  symbol_id: Cell::new(Some(
                    self
                      .ctx
                      .resolved_exports
                      .get(&exported.name)
                      .expect("should have symbol")
                      .local_symbol
                      .symbol,
                  )),
                },
              },
            ),
          ]
          .into_iter(),
          self.ctx.allocator,
        ),
      });
      return oxc::ast::ast::Statement::ModuleDeclaration(Box(self.ctx.allocator.alloc(
        oxc::ast::ast::ModuleDeclaration::ImportDeclaration(Box(import_decl)),
      )));
    } else {
      unimplemented!("fail rewrite export namespace decl with ModuleExportName string")
    }
  }

  pub fn named_export_decl_to_named_import_decl(
    &self,
    named_decl: &oxc::ast::ast::ExportNamedDeclaration,
    source: &oxc::ast::ast::StringLiteral,
  ) -> oxc::ast::ast::Statement<'ast> {
    let import_decl = self.ctx.allocator.alloc(oxc::ast::ast::ImportDeclaration {
      span: Default::default(),
      source: source.clone(),
      assertions: None,
      import_kind: named_decl.export_kind,
      specifiers: Vec::from_iter_in(
        named_decl.specifiers.iter().map(|s| {
          oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(
            oxc::ast::ast::ImportSpecifier {
              span: Default::default(),
              imported: s.local.clone(),
              local: match s.exported {
                oxc::ast::ast::ModuleExportName::Identifier(ref exported) => BindingIdentifier {
                  span: Default::default(),
                  name: exported.name.clone(),
                  symbol_id: Cell::new(Some(
                    self
                      .ctx
                      .resolved_exports
                      .get(&exported.name)
                      .expect("should have symbol")
                      .local_symbol
                      .symbol,
                  )),
                },
                _ => {
                  unimplemented!("fail rewrite export named decl with ModuleExportName string")
                }
              },
            },
          )
        }),
        self.ctx.allocator,
      ),
    });
    Statement::ModuleDeclaration(Box(self.ctx.allocator.alloc(
      oxc::ast::ast::ModuleDeclaration::ImportDeclaration(Box(import_decl)),
    )))
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
          if let Some(source) = &decl.source {
            if self.is_external_request(&source.value) {
              return KEEP_THIS_STMT;
            }
          }
          REMOVE_THIS_STMT
        }
        oxc::ast::ast::ModuleDeclaration::ImportDeclaration(decl) => {
          if self.is_external_request(&decl.source.value) {
            return KEEP_THIS_STMT;
          }
          REMOVE_THIS_STMT
        }
        oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
          if self.is_external_request(&decl.source.value) {
            return KEEP_THIS_STMT;
          }
          REMOVE_THIS_STMT
        }
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

    program.body.iter_mut().for_each(|stmt| {
      if self.should_keep_this_top_level_stmt(stmt) {
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
                  *stmt = Statement::Declaration(Declaration::VariableDeclaration(Box(
                    alloc.alloc(VariableDeclaration {
                      span: Default::default(),
                      kind: oxc::ast::ast::VariableDeclarationKind::Var,
                      declarations,
                      modifiers: Default::default(),
                    }),
                  )))
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
              // external: export { a } from 'a' => import { a } from 'a'
              if let Some(source) = &named_decl.source {
                if self.is_external_request(&source.value) {
                  *stmt = self.named_export_decl_to_named_import_decl(named_decl, source);
                  return;
                }
              }
              if let Some(decl) = &mut named_decl.declaration {
                *stmt = Statement::Declaration(decl.take_in(alloc))
              }
            }
            oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
              // external: export * as a from 'a' => import * as a from 'a'
              if self.is_external_request(&decl.source.value) {
                *stmt = self.namespace_export_decl_to_namespace_import_decl_stmt(decl);
              }
            }
            _ => {}
          }
        }
      } else {
        // replace empty statement to keep stmt order
        *stmt = Statement::EmptyStatement(Box(alloc.alloc(oxc::ast::ast::EmptyStatement {
          span: Default::default(),
        })));
      }
    });

    self.visit_statements(&mut program.body);
  }
}
