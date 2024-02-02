use oxc::{
  allocator::{self, Allocator},
  ast::{
    ast::{self, IdentifierReference, Statement},
    VisitMut,
  },
  span::Atom,
};
use rolldown_common::{ExportsKind, ImportRecordId, ModuleId, SymbolRef, WrapKind};
use rolldown_oxc::{AstSnippet, ExpressionExt, StatementExt, TakeIn};
use rustc_hash::FxHashMap;

use crate::bundler::{
  linker::linker_info::{LinkingInfo, LinkingInfoVec},
  module::{Module, ModuleVec, NormalModule},
  runtime::RuntimeModuleBrief,
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
  pub runtime: &'me RuntimeModuleBrief,
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
  pub fn is_global_identifier_reference(&self, id_ref: &IdentifierReference) -> bool {
    let Some(reference_id) = id_ref.reference_id.get() else {
      // Some `IdentifierReference`s constructed by bundler don't have a `ReferenceId`. They might be global variables.
      // But we don't care about them in this method. This method is only used to check if a `IdentifierReference` from user code is a global variable.
      return false;
    };
    self.scope.is_unresolved(reference_id)
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me Atom {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Atom {
    let symbol = self.ctx.runtime.resolve_symbol(name);
    self.canonical_name_for(symbol)
  }

  fn finalize_import_export_stmt(
    &self,
    _stmt: &Statement<'ast>,
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
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref);
        return Some(self.snippet.var_decl_stmt(
          binding_name_for_wrapper_call_ret.clone(),
          self.snippet.call_expr_expr(wrapper_ref_name.clone()),
        ));
      }
      // Replace the statement with something like `init_foo()`
      WrapKind::Esm => {
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        return Some(self.snippet.call_expr_stmt(wrapper_ref_name.clone()));
      }
    }
    None
  }

  /// return `None` if
  /// - the reference is for a global variable/the reference doesn't have a `SymbolId`
  /// - the reference doesn't have a `ReferenceId`
  /// - the canonical name is the same as the original name
  fn generate_finalized_expr_for_reference(
    &self,
    id_ref: &IdentifierReference,
    is_callee: bool,
  ) -> Option<ast::Expression<'ast>> {
    // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
    let reference_id = id_ref.reference_id.get()?;

    // we will hit this branch if the reference is for a global variable
    let symbol_id = self.scope.symbol_id_for(reference_id)?;

    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbols.get(canonical_ref);

    if let Some(ns_alias) = &symbol.namespace_alias {
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let prop_name = &ns_alias.property_name;
      let access_expr = self
        .snippet
        .literal_prop_access_member_expr_expr(canonical_ns_name.clone(), prop_name.clone());

      return Some(if is_callee {
        let wrapped_callee =
          self.snippet.seq2_in_paren_expr(self.snippet.number_expr(0.0), access_expr);
        wrapped_callee
      } else {
        access_expr
      });
    }

    let canonical_name = self.canonical_name_for(canonical_ref);
    if id_ref.name != canonical_name {
      return Some(self.snippet.id_ref_expr(canonical_name.clone()));
    }

    None
  }
}
// visit

impl<'ast, 'me: 'ast> Finalizer<'me, 'ast> {
  fn visit_top_level_statement_mut(&mut self, stmt: &mut ast::Statement<'ast>) {
    self.visit_statement(stmt);
  }
}

impl<'ast, 'me: 'ast> VisitMut<'ast> for Finalizer<'me, 'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    for directive in program.directives.iter_mut() {
      self.visit_directive(directive);
    }

    let old_body = program.body.take_in(self.alloc);

    old_body.into_iter().for_each(|mut top_stmt| {
      if let Some(import_decl) = top_stmt.as_import_declaration() {
        let rec_id = self.ctx.module.imports[&import_decl.span];
        if let Some(stmt) = self.finalize_import_export_stmt(&top_stmt, rec_id) {
          program.body.push(stmt);
        }
      } else if let Some(export_all_decl) = top_stmt.as_export_all_declaration() {
        let rec_id = self.ctx.module.imports[&export_all_decl.span];
        // "export * as ns from 'path'"
        if let Some(_alias) = &export_all_decl.exported {
          if let Some(stmt) = self.finalize_import_export_stmt(&top_stmt, rec_id) {
            program.body.push(stmt);
          } else {
            return;
          }
        } else {
          // "export * from 'path'"
          // TODO handle this
        }
      } else if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
        match &mut default_decl.declaration {
          ast::ExportDefaultDeclarationKind::Expression(expr) => {
            // "export default foo;" => "var default = foo;"
            let canonical_name_for_default_export_ref =
              self.canonical_name_for(self.ctx.module.default_export_ref);
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
                self.canonical_name_for(self.ctx.module.default_export_ref);
              func.id = Some(self.snippet.id(canonical_name_for_default_export_ref.clone()));
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
                self.canonical_name_for(self.ctx.module.default_export_ref);
              class.id = Some(self.snippet.id(canonical_name_for_default_export_ref.clone()));
            }
            let stmt = ast::Statement::Declaration(ast::Declaration::ClassDeclaration(
              class.take_in(self.alloc),
            ));
            program.body.push(stmt);
          }
          _ => {}
        }
      } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
        if named_decl.source.is_none() {
          if let Some(decl) = &mut named_decl.declaration {
            // `export var foo = 1` => `var foo = 1`
            // `export function foo() {}` => `function foo() {}`
            // `export class Foo {}` => `class Foo {}`
            program.body.push(ast::Statement::Declaration(decl.take_in(self.alloc)));
          } else {
            // `export { foo }`
            // Remove this statement by ignoring it
          }
        } else {
          // `export { foo } from 'path'`
          let rec_id = self.ctx.module.imports[&named_decl.span];
          if let Some(stmt) = self.finalize_import_export_stmt(&top_stmt, rec_id) {
            program.body.push(stmt);
          } else {
            return;
          }
        }
      } else {
        program.body.push(top_stmt);
      }
    });

    for stmt in program.body.iter_mut() {
      self.visit_top_level_statement_mut(stmt);
    }

    // check if we need to add wrapper
    match self.ctx.linking_info.wrap_kind {
      WrapKind::Cjs => {
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
        let commonjs_ref_name = self.canonical_name_for_runtime("__commonJS");
        let old_body = program.body.take_in(self.alloc);

        program.body.push(self.snippet.commonjs_wrapper_stmt(
          wrap_ref_name.clone(),
          commonjs_ref_name.clone(),
          old_body,
        ));
      }
      WrapKind::Esm => {
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
        let esm_ref_name = self.canonical_name_for_runtime("__esm");
        let old_body = program.body.take_in(self.alloc);

        let mut fn_stmts = allocator::Vec::new_in(self.alloc);
        let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);

        // Hoist all top-level "var" and "function" declarations out of the closure
        old_body.into_iter().for_each(|stmt| {
          if stmt.is_function_declaration() {
            fn_stmts.push(stmt);
          } else {
            stmts_inside_closure.push(stmt);
          }
        });
        program.body.extend(fn_stmts);
        program.body.push(self.snippet.esm_wrapper_stmt(
          wrap_ref_name.clone(),
          esm_ref_name.clone(),
          stmts_inside_closure,
        ));
      }
      WrapKind::None => {}
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();

      let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbols.get(canonical_ref);
      assert!(symbol.namespace_alias.is_none());
      let canonical_name = self.canonical_name_for(symbol_ref);
      if ident.name != canonical_name {
        ident.name = canonical_name.clone();
      }
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    if let ast::Expression::Identifier(id_ref) = &mut expr.callee {
      if let Some(new_name) = self.generate_finalized_expr_for_reference(id_ref, true) {
        expr.callee = new_name;
      }
    }

    // visit children
    for arg in expr.arguments.iter_mut() {
      self.visit_argument(arg);
    }
    self.visit_expression(&mut expr.callee);
    if let Some(parameters) = &mut expr.type_parameters {
      self.visit_ts_type_parameter_instantiation(parameters);
    }
  }

  #[allow(clippy::collapsible_else_if)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let Some(call_expr) = expr.as_call_expression() {
      if let ast::Expression::Identifier(callee) = &call_expr.callee {
        if callee.name == "require" && self.is_global_identifier_reference(callee) {
          let rec_id = self.ctx.module.imports[&call_expr.span];
          let rec = &self.ctx.module.import_records[rec_id];
          if let Module::Normal(importee) = &self.ctx.modules[rec.resolved_module] {
            let importee_linking_info = &self.ctx.linking_infos[importee.id];
            let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
            if matches!(importee.exports_kind, ExportsKind::CommonJs) {
              *expr = self.snippet.call_expr_expr(wrap_ref_name.clone());
            } else {
              let ns_name = self.canonical_name_for(importee.namespace_symbol);
              let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
              *expr = self.snippet.seq2_in_paren_expr(
                self.snippet.call_expr_expr(wrap_ref_name.clone()),
                self.snippet.call_expr_with_arg_expr(to_commonjs_ref_name.clone(), ns_name.clone()),
              );
            }
          }
        }
      }
    }

    if let Some(id_ref) = expr.as_identifier() {
      if let Some(new_expr) = self.generate_finalized_expr_for_reference(id_ref, false) {
        *expr = new_expr;
      }
    }

    // visit children
    self.visit_expression_match(expr);
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
    // rewrite `const val = { a };` to `const val = { a: a.xxx }`
    match prop.value {
      ast::Expression::Identifier(ref id_ref) if prop.shorthand => {
        if let Some(expr) = self.generate_finalized_expr_for_reference(id_ref, true) {
          prop.value = expr;
          prop.shorthand = false;
        }
      }
      _ => {}
    }

    // visit children
    self.visit_property_key(&mut prop.key);
    self.visit_expression(&mut prop.value);
    if let Some(init) = &mut prop.init {
      self.visit_expression(init);
    }
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    // visit children
    for prop in pat.properties.iter_mut() {
      match &mut prop.value.kind {
        // Rewrite `const { a } = obj;`` to `const { a: a$1 } = obj;`
        ast::BindingPatternKind::BindingIdentifier(ident) if prop.shorthand => {
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name {
              ident.name = canonical_name.clone();
              prop.shorthand = false;
            }
          }
        }
        // Rewrite `const { a = 1 } = obj;`` to `const { a: a$1 = 1 } = obj;`
        ast::BindingPatternKind::AssignmentPattern(assign_pat)
          if prop.shorthand
            && matches!(assign_pat.left.kind, ast::BindingPatternKind::BindingIdentifier(_)) =>
        {
          let ast::BindingPatternKind::BindingIdentifier(ident) = &mut assign_pat.left.kind else {
            unreachable!()
          };
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name {
              ident.name = canonical_name.clone();
              prop.shorthand = false;
            }
          }
        }
        _ => {}
      }
    }

    // visit children
    for prop in pat.properties.iter_mut() {
      self.visit_binding_property(prop);
    }
    if let Some(rest) = &mut pat.rest {
      self.visit_rest_element(rest);
    }
  }
}
