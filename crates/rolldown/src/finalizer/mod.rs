use oxc::{
  allocator::Allocator,
  ast::ast::{self, IdentifierReference, Statement},
  span::{Atom, SPAN},
};
use rolldown_common::{AstScope, ImportRecordId, ModuleId, SymbolRef, WrapKind};
use rolldown_oxc_utils::{AstSnippet, BindingPatternExt, Dummy, IntoIn, TakeIn};

mod finalizer_context;
mod impl_visit_mut_for_finalizer;
pub use finalizer_context::FinalizerContext;
use rolldown_rstr::Rstr;
mod rename;

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

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me Rstr {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
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
                ..Dummy::dummy(self.alloc)
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
              ..Dummy::dummy(self.alloc)
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
                ..Dummy::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ),
            ..Dummy::dummy(self.alloc)
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

  fn generate_namespace_variable_declaration(&self) -> Vec<ast::Statement<'ast>> {
    let ns_name = self.canonical_name_for(self.ctx.module.namespace_symbol);
    // construct `var ns_name = {}`
    let namespace_decl_stmt = self
      .snippet
      .var_decl_stmt(ns_name, ast::Expression::ObjectExpression(Dummy::dummy(self.alloc)));

    let exports_len = self.ctx.linking_info.canonical_exports_len();

    if exports_len == 0 {
      return vec![namespace_decl_stmt];
    }

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.alloc);
    arg_obj_expr.properties.reserve_exact(exports_len);

    self.ctx.linking_info.canonical_exports().for_each(|(export, resolved_export)| {
      // prop_name: () => returned
      let prop_name = export;
      let returned = self.generate_finalized_expr_for_symbol_ref(resolved_export.symbol_ref);
      arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
        ast::ObjectProperty {
          key: ast::PropertyKey::Identifier(
            self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
          ),
          value: self.snippet.only_return_arrow_expr(returned),
          ..Dummy::dummy(self.alloc)
        }
        .into_in(self.alloc),
      ));
    });

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let mut export_call_expr = self.snippet.call_expr(self.canonical_name_for_runtime("__export"));
    export_call_expr
      .arguments
      .push(ast::Argument::Expression(self.snippet.id_ref_expr(ns_name, SPAN)));
    export_call_expr.arguments.push(ast::Argument::Expression(ast::Expression::ObjectExpression(
      arg_obj_expr.into_in(self.alloc),
    )));
    let export_call_stmt = ast::Statement::ExpressionStatement(
      ast::ExpressionStatement {
        expression: ast::Expression::CallExpression(export_call_expr.into_in(self.alloc)),
        ..Dummy::dummy(self.alloc)
      }
      .into_in(self.alloc),
    );

    vec![namespace_decl_stmt, export_call_stmt]
  }
}
