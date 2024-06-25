use oxc::{
  allocator::Allocator,
  ast::ast::{self, IdentifierReference, Statement},
  semantic::SymbolId,
  span::{Atom, SPAN},
};
use rolldown_common::{AstScopes, ImportRecordId, ModuleId, SymbolRef, WrapKind};
use rolldown_oxc_utils::{AstSnippet, BindingPatternExt, IntoIn, TakeIn};

mod finalizer_context;
mod impl_visit_mut;
pub use finalizer_context::ScopeHoistingFinalizerContext;
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::is_validate_identifier_name;

use crate::types::tree_shake::UsedInfo;
mod rename;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
}

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  pub fn is_global_identifier_reference(&self, id_ref: &IdentifierReference) -> bool {
    let Some(reference_id) = id_ref.reference_id.get() else {
      // Some `IdentifierReference`s constructed by bundler don't have a `ReferenceId`. They might be global variables.
      // But we don't care about them in this method. This method is only used to check if a `IdentifierReference` from user code is a global variable.
      return false;
    };
    self.scope.is_unresolved(reference_id)
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me Rstr {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  /// Used for member expression access with ambiguous name.
  pub fn try_canonical_name_for(&self, symbol: SymbolRef) -> Option<&'me Rstr> {
    self.ctx.canonical_names.get(&symbol)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Rstr {
    let symbol = self.ctx.runtime.resolve_symbol(name);
    self.canonical_name_for(symbol)
  }

  fn should_remove_import_export_stmt(
    &self,
    stmt: &mut Statement<'ast>,
    rec_id: ImportRecordId,
  ) -> bool {
    let rec = &self.ctx.module.import_records[rec_id];
    let ModuleId::Normal(importee_id) = rec.resolved_module else {
      return true;
    };
    let importee_linking_info = &self.ctx.linking_infos[importee_id];
    match importee_linking_info.wrap_kind {
      WrapKind::None => {
        // Remove this statement by ignoring it
      }
      WrapKind::Cjs => {
        // Replace the statement with something like `var import_foo = __toESM(require_foo())`
        let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref);
        *stmt = self.snippet.var_decl_stmt(
          binding_name_for_wrapper_call_ret,
          self.snippet.call_expr_with_arg_expr_expr(
            to_esm_fn_name,
            self.snippet.call_expr_expr(wrapper_ref_name),
          ),
        );
        return false;
      }
      // Replace the statement with something like `init_foo()`
      WrapKind::Esm => {
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        *stmt = self.snippet.call_expr_stmt(wrapper_ref_name);
        return false;
      }
    }
    true
  }

  fn generate_finalized_expr_for_symbol_ref(&self, symbol_ref: SymbolRef) -> ast::Expression<'ast> {
    let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbols.get(canonical_ref);

    if let Some(ns_alias) = &symbol.namespace_alias {
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let prop_name = &ns_alias.property_name;
      let access_expr =
        self.snippet.literal_prop_access_member_expr_expr(canonical_ns_name, prop_name);

      access_expr
    } else {
      let canonical_name = self.canonical_name_for(canonical_ref);
      self.snippet.id_ref_expr(canonical_name, SPAN)
    }
  }

  fn convert_decl_to_assignment(
    &self,
    decl: &mut ast::Declaration<'ast>,
    hoisted_names: &mut Vec<Atom<'ast>>,
  ) -> Option<ast::Statement<'ast>> {
    match decl {
      ast::Declaration::VariableDeclaration(var_decl) => {
        let mut seq_expr = ast::SequenceExpression::dummy(self.alloc);
        var_decl.declarations.iter_mut().for_each(|var_decl| {
          var_decl.id.binding_identifiers().iter().for_each(|id| {
            hoisted_names.push(id.name.clone());
          });
          // Turn `var ... = ...` to `... = ...`
          if let Some(init_expr) = &mut var_decl.init {
            let left = var_decl.id.take_in(self.alloc).into_assignment_target(self.alloc);
            seq_expr.expressions.push(ast::Expression::AssignmentExpression(
              ast::AssignmentExpression {
                left,
                right: init_expr.take_in(self.alloc),
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ));
          };
        });
        if seq_expr.expressions.is_empty() {
          None
        } else {
          Some(ast::Statement::ExpressionStatement(
            ast::ExpressionStatement {
              expression: ast::Expression::SequenceExpression(seq_expr.into_in(self.alloc)),
              ..TakeIn::dummy(self.alloc)
            }
            .into_in(self.alloc),
          ))
        }
      }
      ast::Declaration::ClassDeclaration(cls_decl) => {
        let cls_name = cls_decl.id.take().expect("should have a name at this point").name;
        hoisted_names.push(cls_name.clone());
        // Turn `class xxx {}` to `xxx = class {}`
        Some(ast::Statement::ExpressionStatement(
          ast::ExpressionStatement {
            expression: ast::Expression::AssignmentExpression(
              ast::AssignmentExpression {
                left: self.snippet.simple_id_assignment_target(&cls_name, cls_decl.span),
                right: ast::Expression::ClassExpression(cls_decl.take_in(self.alloc)),
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ),
            ..TakeIn::dummy(self.alloc)
          }
          .into_in(self.alloc),
        ))
      }
      ast::Declaration::FunctionDeclaration(_) => {
        // Function declaration itself as a whole will be hoisted, so we don't need to convert it to an assignment.
        None
      }
      ast::Declaration::UsingDeclaration(_) => {
        todo!("`using` declaration is not supported yet")
      }
      _ => unreachable!("TypeScript code should be preprocessed"),
    }
  }

  fn generate_declaration_of_module_namespace_object(&self) -> Vec<ast::Statement<'ast>> {
    let var_name = self.canonical_name_for(self.ctx.module.namespace_object_ref);
    // construct `var ns_name = {}`
    let decl_stmt = self
      .snippet
      .var_decl_stmt(var_name, ast::Expression::ObjectExpression(TakeIn::dummy(self.alloc)));

    let exports_len = self.ctx.linking_info.used_canonical_exports().count();
    if exports_len == 0 {
      let can_not_be_eliminated =
        self.ctx.linking_info.used_exports_info.used_info.contains(UsedInfo::USED_AS_NAMESPACE)
          || self.ctx.module.import_records.iter().any(|record| {
            let id = record.resolved_module;

            if let Some(normal_module_id) = id.as_normal() {
              let m = &self.ctx.modules[normal_module_id];
              !m.exports_kind.is_esm()
            } else {
              true
            }
          });
      if can_not_be_eliminated {
        return vec![decl_stmt];
      }
      return vec![];
    }

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.alloc);
    arg_obj_expr.properties.reserve_exact(exports_len);

    self.ctx.linking_info.used_canonical_exports().for_each(|(export, resolved_export)| {
      // prop_name: () => returned
      let prop_name = export;
      let returned = self.generate_finalized_expr_for_symbol_ref(resolved_export.symbol_ref);
      arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
        ast::ObjectProperty {
          key: if is_validate_identifier_name(prop_name) {
            ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
            )
          } else {
            ast::PropertyKey::StringLiteral(
              self.snippet.string_literal(prop_name, SPAN).into_in(self.alloc),
            )
          },
          value: self.snippet.only_return_arrow_expr(returned),
          ..TakeIn::dummy(self.alloc)
        }
        .into_in(self.alloc),
      ));
    });

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let mut export_call_expr = self.snippet.call_expr(self.canonical_name_for_runtime("__export"));
    export_call_expr.arguments.push(ast::Argument::from(self.snippet.id_ref_expr(var_name, SPAN)));
    export_call_expr
      .arguments
      .push(ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc)));
    let export_call_stmt = ast::Statement::ExpressionStatement(
      ast::ExpressionStatement {
        expression: ast::Expression::CallExpression(export_call_expr.into_in(self.alloc)),
        ..TakeIn::dummy(self.alloc)
      }
      .into_in(self.alloc),
    );

    vec![decl_stmt, export_call_stmt]
  }

  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    id_ref.reference_id.get().and_then(|ref_id| self.scope.symbol_id_for(ref_id))
  }
}
