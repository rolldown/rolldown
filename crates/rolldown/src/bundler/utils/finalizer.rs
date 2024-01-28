use oxc::{
  allocator::Allocator,
  ast::{
    ast::{self, Statement},
    VisitMut,
  },
  span::Atom,
};
use rolldown_common::{ImportRecordId, ModuleId, SymbolRef, WrapKind};
use rolldown_oxc::{AstSnippet, IntoIn, StatementExt, TakeIn};
use rustc_hash::FxHashMap;

use crate::bundler::{
  linker::linker_info::{LinkingInfo, LinkingInfoVec},
  module::{ModuleVec, NormalModule},
};

use super::{ast_scope::AstScope, symbols::Symbols};

pub struct FinalizerContext<'me> {
  pub id: ModuleId,
  pub module: &'me NormalModule,
  pub modules: &'me ModuleVec,
  pub linking_info: &'me LinkingInfo,
  pub linking_infos: &'me LinkingInfoVec,
  pub symbols: &'me Symbols,
  pub canonical_names: &'me FxHashMap<SymbolRef, Atom>,
}

pub struct Finalizer<'me, 'ast> {
  pub alloc: &'ast Allocator,
  pub ctx: FinalizerContext<'me>,
  pub scope: &'me AstScope,
  pub snippet: &'me AstSnippet<'ast>,
}

impl<'me, 'ast> Finalizer<'me, 'ast>
where
  'me: 'ast,
{
  pub fn canonical_name_for(&self, symbol: SymbolRef) -> Option<&'me Atom> {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  // pub fn canonical_name_for_runtime(&self, name: &str) -> &Atom {
  //   let symbol = self.ctx.graph.runtime.resolve_symbol(&Atom::new_inline(name));
  //   self.canonical_name_for(symbol)
  // }

  #[allow(clippy::needless_pass_by_value)]
  fn finalize_import_export_stmt(
    &self,
    _stmt: Statement<'ast>,
    rec_id: ImportRecordId,
  ) -> Option<Statement<'ast>> {
    let rec = &self.ctx.module.import_records[rec_id];
    let importee_id = rec.resolved_module;
    let importee_linking_info = &self.ctx.linking_infos[importee_id];
    match importee_linking_info.wrap_kind {
      WrapKind::None => {
        // Remove this statement by ignoring it
      }
      WrapKind::Cjs => {
        // Replace the statement with something like `var import_foo = require_foo()`
        let wrapper_ref_name =
          self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap()).unwrap();
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref).unwrap();
        return Some(self.snippet.var_decl_stmt(
          binding_name_for_wrapper_call_ret.clone(),
          self.snippet.call_expr_expr(wrapper_ref_name.clone()),
        ));
      }
      // Replace the statement with something like `init_foo()`
      WrapKind::Esm => {
        let wrapper_ref_name =
          self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap()).unwrap();
        return Some(self.snippet.call_expr_stmt(wrapper_ref_name.clone()));
      }
    }
    None
  }
}
// visit

impl<'ast, 'me: 'ast> Finalizer<'me, 'ast> {
  fn visit_top_level_statement_mut(&mut self, stmt: &mut ast::Statement<'ast>) {
    // FIXME: this is a hack to avoid renaming import statements
    if stmt.is_import_declaration() {
      return;
    }

    self.visit_statement(stmt);
  }
}

impl<'ast, 'me: 'ast> VisitMut<'ast> for Finalizer<'me, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    for directive in program.directives.iter_mut() {
      self.visit_directive(directive);
    }

    let old_body = program.body.take_in(self.alloc);

    old_body.into_iter().for_each(|mut top_stmt| {
      if let Some(import_decl) = top_stmt.as_import_declaration() {
        let rec_id = self.ctx.module.imports[&import_decl.span];
        if let Some(stmt) = self.finalize_import_export_stmt(top_stmt, rec_id) {
          program.body.push(stmt);
        }
      } else if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
        match &mut default_decl.declaration {
          ast::ExportDefaultDeclarationKind::Expression(expr) => {
            // "export default foo;" => "var default = foo;"
            let canonical_name_for_default_export_ref =
              self.canonical_name_for(self.ctx.module.default_export_ref).unwrap();
            program.body.push(self.snippet.var_decl_stmt(
              canonical_name_for_default_export_ref.clone(),
              expr.take_in(self.alloc),
            ));
          }
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            // "export default function() {}" => "function default() {}"
            // "export default function foo() {}" => "function foo() {}"
            if func.id.is_none() {
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref).unwrap();
              func.id = Some(self.snippet.binding(canonical_name_for_default_export_ref.clone()));
            }
            let stmt = ast::Statement::Declaration(ast::Declaration::FunctionDeclaration(
              func.take_in(self.alloc),
            ));
            program.body.push(stmt);
          }
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            // "export default class {}" => "class default {}"
            // "export default class Foo {}" => "class Foo {}"
            if class.id.is_none() {
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref).unwrap();
              class.id = Some(self.snippet.binding(canonical_name_for_default_export_ref.clone()));
            }
            let stmt = ast::Statement::Declaration(ast::Declaration::ClassDeclaration(
              class.take_in(self.alloc),
            ));
            program.body.push(stmt);
          }
          _ => {}
        }
      } else {
        program.body.push(top_stmt);
      }
    });

    for stmt in program.body.iter_mut() {
      self.visit_top_level_statement_mut(stmt);
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();

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
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  #[allow(clippy::collapsible_else_if)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let ast::Expression::Identifier(id_ref) = expr {
      if let Some(reference_id) = id_ref.reference_id.get() {
        if let Some(symbol_id) = self.scope.symbol_id_for(reference_id) {
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
        } else {
          // we will hit this branch if the reference is for a global variable
        };
      } else {
        // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
      }
    } else {
      self.visit_expression_match(expr);
    }
  }
}
