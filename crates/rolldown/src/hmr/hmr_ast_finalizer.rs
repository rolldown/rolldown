use oxc::{
  allocator::{Allocator, Box as ArenaBox, Dummy, IntoIn, TakeIn},
  ast::{
    NONE,
    ast::{
      self, ExportDefaultDeclarationKind, Expression, ObjectExpression, ObjectPropertyKind,
      PropertyKind, Statement,
    },
  },
  ast_visit::{VisitMut, walk_mut},
  semantic::{Scoping, SymbolId},
  span::{SPAN, Span},
};

use rolldown_common::{ImportRecordIdx, IndexModules, Module, ModuleIdx, NormalModule};
use rolldown_ecmascript_utils::{
  AstSnippet, BindingIdentifierExt, ExpressionExt, quote_expr, quote_stmts,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

pub struct HmrAstFinalizer<'me, 'ast> {
  // Outside input
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub scoping: &'me Scoping,
  pub modules: &'me IndexModules,
  pub module: &'me NormalModule,
  pub affected_module_idx_to_init_fn_name: &'me FxHashMap<ModuleIdx, String>,
  //Internal state
  pub import_binding: FxHashMap<SymbolId, String>,
  pub exports: oxc::allocator::Vec<'ast, ObjectPropertyKind<'ast>>,
  pub dependencies: FxIndexSet<ModuleIdx>,
  pub imports: FxHashSet<ModuleIdx>,
}

impl<'ast> HmrAstFinalizer<'_, 'ast> {
  pub fn generate_runtime_module_register_for_hmr(&mut self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];

    let module_exports =
      match self.module.exports_kind {
        rolldown_common::ExportsKind::Esm => {
          let binding_name_for_namespace_object_ref = format!("ns_{}", self.module.repr_name);

          ret.extend(self.generate_declaration_of_module_namespace_object(
            &binding_name_for_namespace_object_ref,
          ));

          // { exports: namespace }
          ast::Argument::ObjectExpression(self.snippet.builder.alloc_object_expression(
            SPAN,
            self.snippet.builder.vec1(self.snippet.builder.object_property_kind_object_property(
              SPAN,
              PropertyKind::Init,
              self.snippet.builder.property_key_static_identifier(SPAN, "exports"),
              self.snippet.id_ref_expr(&binding_name_for_namespace_object_ref, SPAN),
              true,
              false,
              false,
            )),
          ))
        }
        rolldown_common::ExportsKind::CommonJs => {
          // `module`
          ast::Argument::Identifier(self.snippet.builder.alloc_identifier_reference(SPAN, "module"))
        }
        rolldown_common::ExportsKind::None => ast::Argument::ObjectExpression(
          // `{}`
          self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec()),
        ),
      };

    // __rolldown_runtime__.registerModule(moduleId, module)
    let arguments = self.snippet.builder.vec_from_array([
      ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
        SPAN,
        self.snippet.builder.atom(&self.module.stable_id),
        None,
      )),
      module_exports,
    ]);

    let register_call = self.snippet.builder.alloc_call_expression(
      SPAN,
      self.snippet.id_ref_expr("__rolldown_runtime__.registerModule", SPAN),
      NONE,
      arguments,
      false,
    );

    ret.push(ast::Statement::ExpressionStatement(
      self
        .snippet
        .builder
        .alloc_expression_statement(SPAN, ast::Expression::CallExpression(register_call)),
    ));

    ret
  }

  pub fn rewrite_hot_accept_call_deps(&self, call_expr: &mut ast::CallExpression<'ast>) {
    // Check whether the callee is `import.meta.hot.accept`.
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    if call_expr.arguments.is_empty() {
      // `import.meta.hot.accept()`
      return;
    }

    match &mut call_expr.arguments[0] {
      ast::Argument::StringLiteral(string_literal) => {
        // `import.meta.hot.accept('./dep.js', ...)`
        let import_record = &self.module.import_records
          [self.module.hmr_info.module_request_to_import_record_idx[string_literal.value.as_str()]];
        string_literal.value =
          self.snippet.builder.atom(self.modules[import_record.resolved_module].stable_id());
      }
      ast::Argument::ArrayExpression(array_expression) => {
        // `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`
        array_expression.elements.iter_mut().for_each(|element| {
          if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
            let import_record =
              &self.module.import_records[self.module.hmr_info.module_request_to_import_record_idx
                [string_literal.value.as_str()]];
            string_literal.value =
              self.snippet.builder.atom(self.modules[import_record.resolved_module].stable_id());
          }
        });
      }
      _ => {}
    }
  }

  pub fn rewrite_import_meta_hot(&self, expr: &mut ast::Expression<'ast>) {
    if expr.is_import_meta_hot() {
      let hot_name = format!("hot_{}", self.module.repr_name);
      *expr = self.snippet.id_ref_expr(&hot_name, SPAN);
    }
  }

  fn create_binding_name(importee: &Module, rec_id: ImportRecordIdx) -> String {
    format!("import_{}_{}", importee.repr_name(), rec_id.raw())
  }

  fn create_load_exports_call_stmt(
    &mut self,
    importee: &Module,
    binding_name: &str,
    span: Span,
  ) -> Option<Statement<'ast>> {
    if self.imports.contains(&importee.idx()) {
      return None;
    }
    self.imports.insert(importee.idx());

    let id = &importee.stable_id();
    let interop = match importee {
      Module::Normal(importee) => self.module.interop(importee),
      Module::External(_) => None,
    };
    let call_expr =
      quote_expr(self.alloc, format!("__rolldown_runtime__.loadExports({id:?});",).as_str());

    let stmt = self.snippet.variable_declarator_require_call_stmt(
      binding_name,
      self.snippet.to_esm_call_with_interop("__rolldown_runtime__.__toESM", call_expr, interop),
      span,
    );
    Some(stmt)
  }

  fn generate_declaration_of_module_namespace_object(
    &mut self,
    binding_name_for_namespace_object_ref: &str,
  ) -> Vec<ast::Statement<'ast>> {
    // construct `var [binding_name_for_namespace_object_ref] = {}`
    let decl_stmt = self.snippet.var_decl_stmt(
      binding_name_for_namespace_object_ref,
      ast::Expression::ObjectExpression(ArenaBox::new_in(
        ObjectExpression::dummy(self.alloc),
        self.alloc,
      )),
    );

    // TODO reexport external module

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = self
      .snippet
      .builder
      .alloc_object_expression(SPAN, self.snippet.builder.vec_with_capacity(self.exports.len()));
    arg_obj_expr.properties.extend(self.exports.drain(..));

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let export_call_expr = self.snippet.builder.expression_call(
      SPAN,
      self.snippet.id_ref_expr("__rolldown_runtime__.__export", SPAN),
      NONE,
      self.snippet.builder.vec_from_array([
        ast::Argument::from(self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN)),
        ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc)),
      ]),
      false,
    );
    let export_call_stmt = self.snippet.builder.statement_expression(SPAN, export_call_expr);

    vec![decl_stmt, export_call_stmt]
  }
}

impl<'ast> VisitMut<'ast> for HmrAstFinalizer<'_, 'ast> {
  fn visit_program(&mut self, it: &mut ast::Program<'ast>) {
    walk_mut::walk_program(self, it);
    // Move the original program body to a try catch block to unlock the ability to be error-tolerant
    let mut try_block =
      self.snippet.builder.alloc_block_statement(SPAN, self.snippet.builder.vec());

    let dependencies_init_fns = self
      .dependencies
      .iter()
      .filter_map(|dep| self.affected_module_idx_to_init_fn_name.get(dep))
      .map(|fn_name| format!("{fn_name}();"))
      .collect::<Vec<_>>()
      .join("\n");

    let dependencies_init_fn_stmts = quote_stmts(self.alloc, dependencies_init_fns.as_str());

    let runtime_module_register = self.generate_runtime_module_register_for_hmr();

    try_block.body.reserve_exact(
      runtime_module_register.len() + it.body.len() + dependencies_init_fn_stmts.len() + 1 /* import.meta.hot*/,
    );
    try_block.body.extend(runtime_module_register);
    try_block.body.extend(dependencies_init_fn_stmts);
    try_block.body.push(self.snippet.stmt_of_init_module_hot_context(
      &format!("hot_{}", self.module.repr_name),
      &self.module.stable_id,
    ));
    try_block.body.extend(it.body.take_in(self.alloc));

    let final_block = self.snippet.builder.alloc_block_statement(SPAN, self.snippet.builder.vec());

    let try_stmt =
      self.snippet.builder.alloc_try_statement(SPAN, try_block, NONE, Some(final_block));

    let init_fn_name = &self.affected_module_idx_to_init_fn_name[&self.module.idx];

    // function () { [user code] }
    let user_code_wrapper =
      ast::Expression::FunctionExpression(self.snippet.builder.alloc_function(
        SPAN,
        ast::FunctionType::FunctionExpression,
        None,
        false,
        false,
        false,
        NONE,
        NONE,
        self.snippet.builder.formal_parameters(
          SPAN,
          ast::FormalParameterKind::Signature,
          self.snippet.builder.vec_with_capacity(2),
          NONE,
        ),
        NONE,
        Some(self.snippet.builder.function_body(
          SPAN,
          self.snippet.builder.vec(),
          self.snippet.builder.vec1(ast::Statement::TryStatement(try_stmt)),
        )),
      ));

    // var init_foo = __rolldown__runtime.createEsmInitializer(function () { [user code] })
    let var_decl = self.snippet.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.snippet.builder.vec1(
        self.snippet.builder.variable_declarator(
          SPAN,
          ast::VariableDeclarationKind::Var,
          self.snippet.builder.binding_pattern(
            ast::BindingPatternKind::BindingIdentifier(
              self
                .snippet
                .builder
                .alloc_binding_identifier(SPAN, self.snippet.builder.atom(init_fn_name)),
            ),
            NONE,
            false,
          ),
          Some(ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
            SPAN,
            self.snippet.id_ref_expr("__rolldown_runtime__.createEsmInitializer", SPAN),
            NONE,
            self.snippet.builder.vec1(ast::Argument::from(user_code_wrapper)),
            false,
          ))),
          false,
        ),
      ),
      false,
    );

    it.body.push(ast::Statement::VariableDeclaration(var_decl));
  }

  #[expect(clippy::too_many_lines)]
  fn visit_statement(&mut self, node: &mut ast::Statement<'ast>) {
    if let Some(module_decl) = node.as_module_declaration_mut() {
      match module_decl {
        ast::ModuleDeclaration::ImportDeclaration(import_decl) => {
          // Transform
          // ```js
          // import foo, { bar } from './foo.js';
          // console.log(foo, bar);
          // ```
          // to
          // ```js
          // const import_foo = __rolldown_runtime__.loadExports('./foo.js');
          // console.log(import_foo.default, import_foo.bar);
          // ```
          let rec_id = self.module.imports[&import_decl.span];
          let rec = &self.module.import_records[rec_id];
          let importee = &self.modules[rec.resolved_module];
          self.dependencies.insert(rec.resolved_module);

          let binding_name = Self::create_binding_name(importee, rec_id);
          import_decl.specifiers.as_ref().inspect(|specifiers| {
            specifiers.iter().for_each(|spec| match spec {
              ast::ImportDeclarationSpecifier::ImportSpecifier(import_specifier) => {
                self.import_binding.insert(
                  import_specifier.local.expect_symbol_id(),
                  format!("{binding_name}.{}", import_specifier.imported.name()),
                );
              }
              ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(import_default_specifier) => {
                self.import_binding.insert(
                  import_default_specifier.local.expect_symbol_id(),
                  format!("{binding_name}.default"),
                );
              }
              ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
                import_namespace_specifier,
              ) => {
                self.import_binding.insert(
                  import_namespace_specifier.local.expect_symbol_id(),
                  binding_name.to_string(),
                );
              }
            });
          });
          if let Some(stmt) =
            self.create_load_exports_call_stmt(importee, &binding_name, import_decl.span)
          {
            *node = stmt;
          } else {
            *node =
              ast::Statement::EmptyStatement(self.snippet.builder.alloc_empty_statement(SPAN));
          }
        }
        ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
          if let Some(_source) = &decl.source {
            // export {} from '...'
            let rec_id = self.module.imports[&decl.span];
            let rec = &self.module.import_records[rec_id];
            let importee = &self.modules[rec.resolved_module];
            self.dependencies.insert(rec.resolved_module);

            let binding_name = Self::create_binding_name(importee, rec_id);
            self.exports.extend(decl.specifiers.iter().map(|specifier| {
              self.snippet.object_property_kind_object_property(
                &specifier.exported.name(),
                match &specifier.local {
                  ast::ModuleExportName::IdentifierName(ident) => {
                    Expression::StaticMemberExpression(
                      self.snippet.builder.alloc_static_member_expression(
                        SPAN,
                        self.snippet.id_ref_expr(&binding_name, SPAN),
                        self.snippet.builder.identifier_name(SPAN, ident.name.as_str()),
                        false,
                      ),
                    )
                  }
                  ast::ModuleExportName::StringLiteral(str) => {
                    Expression::ComputedMemberExpression(
                      self.snippet.builder.alloc_computed_member_expression(
                        SPAN,
                        self.snippet.id_ref_expr(&binding_name, SPAN),
                        self.snippet.builder.expression_string_literal(
                          SPAN, str.value.as_str(), None
                        ),
                        false,
                      ),
                    )
                  }
                  ast::ModuleExportName::IdentifierReference(_) => {
                    unreachable!(
                      "ModuleExportName IdentifierReference is invalid in ExportNamedDeclaration with source"
                    )
                  }
                },
                matches!(specifier.exported, ast::ModuleExportName::StringLiteral(_))
              )
            }));
            if let Some(stmt) =
              self.create_load_exports_call_stmt(importee, &binding_name, decl.span)
            {
              *node = stmt;
            } else {
              *node =
                ast::Statement::EmptyStatement(self.snippet.builder.alloc_empty_statement(SPAN));
            }
          } else if let Some(decl) = &mut decl.declaration {
            match decl {
              ast::Declaration::VariableDeclaration(var_decl) => {
                // export var foo = 1
                // export var { foo, bar } = { foo: 1, bar: 2 }
                self.exports.extend(var_decl.declarations.iter().filter_map(|decl| {
                  decl.id.get_identifier_name().map(|ident| {
                    self.snippet.object_property_kind_object_property(
                      ident.as_str(),
                      self.snippet.id_ref_expr(ident.as_str(), SPAN),
                      false,
                    )
                  })
                }));
              }
              ast::Declaration::FunctionDeclaration(fn_decl) => {
                // export function foo() {}
                let id = fn_decl.id.as_ref().unwrap().name.as_str();
                self.exports.push(self.snippet.object_property_kind_object_property(
                  id,
                  self.snippet.id_ref_expr(id, SPAN),
                  false,
                ));
              }
              ast::Declaration::ClassDeclaration(cls_decl) => {
                // export class Foo {}
                let id = cls_decl.id.as_ref().unwrap().name.as_str();
                self.exports.push(self.snippet.object_property_kind_object_property(
                  id,
                  self.snippet.id_ref_expr(id, SPAN),
                  false,
                ));
              }
              _ => unreachable!("doesn't support ts now"),
            }
            *node = ast::Statement::from(decl.take_in(self.alloc));
          } else {
            // export { foo, bar as bar2 }
            self.exports.extend(decl.specifiers.iter().map(|specifier| {
              self.snippet.object_property_kind_object_property(
                &specifier.exported.name(),
                self.snippet.id_ref_expr(&specifier.local.name(), SPAN),
                matches!(specifier.exported, ast::ModuleExportName::StringLiteral(_)),
              )
            }));
            *node =
              ast::Statement::EmptyStatement(self.snippet.builder.alloc_empty_statement(SPAN));
          }
        }
        ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => match &mut decl.declaration {
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
              self.exports.push(self.snippet.object_property_kind_object_property(
                "default",
                self.snippet.id_ref_expr(&id.name, SPAN),
                false,
              ));
            } else {
              function.id = Some(self.snippet.id("__rolldown_default__", SPAN));
              self.exports.push(self.snippet.object_property_kind_object_property(
                "default",
                self.snippet.id_ref_expr("__rolldown_default__", SPAN),
                false,
              ));
            }
            *node = ast::Statement::FunctionDeclaration(ArenaBox::new_in(
              function.as_mut().take_in(self.alloc),
              self.alloc,
            ));
          }
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
              self.exports.push(self.snippet.object_property_kind_object_property(
                "default",
                self.snippet.id_ref_expr(&id.name, SPAN),
                false,
              ));
            } else {
              class.id = Some(self.snippet.id("__rolldown_default__", SPAN));
              self.exports.push(self.snippet.object_property_kind_object_property(
                "default",
                self.snippet.id_ref_expr("__rolldown_default__", SPAN),
                false,
              ));
            }
            *node = ast::Statement::ClassDeclaration(ArenaBox::new_in(
              class.as_mut().take_in(self.alloc),
              self.alloc,
            ));
          }
          expr @ ast::match_expression!(ExportDefaultDeclarationKind) => {
            let expr = expr.to_expression_mut();
            // Transform `export default [expression]` => `var __rolldown_default__ = [expression]`
            *node = self.snippet.var_decl_stmt("__rolldown_default__", expr.take_in(self.alloc));
            self.exports.push(self.snippet.object_property_kind_object_property(
              "default",
              self.snippet.id_ref_expr("__rolldown_default__", SPAN),
              false,
            ));
          }
          unhandled_kind => {
            unreachable!("Unexpected export default declaration kind: {unhandled_kind:#?}");
          }
        },
        _ => {
          // TODO(hyf0): Handle other module declarations
          // e.g. reexport, export, etc.
        }
      }
    }

    // For `require` statements
    // Transform
    // ```js
    // const foo = require('./foo.js');
    // console.log(foo);
    // ```
    // to
    // ```js
    // const foo = __rolldown_runtime__.loadExports('./foo.js');
    // console.log(foo);
    // ```

    // For `import()` statements
    // Transform
    // ```js
    // const foo = await import('./foo.js');
    // console.log(foo);
    // ```
    // to
    // ```js
    // const foo = await Promise.resolve(__rolldown_runtime__.loadExports('./foo.js'));
    // console.log(foo);
    // ```

    walk_mut::walk_statement(self, node);
  }

  fn visit_expression(&mut self, it: &mut ast::Expression<'ast>) {
    if let Some(ident) = it.as_identifier() {
      if let Some(reference_id) = ident.reference_id.get() {
        let reference = self.scoping.get_reference(reference_id);
        if let Some(symbol_id) = reference.symbol_id() {
          if let Some(binding_name) = self.import_binding.get(&symbol_id) {
            *it = self.snippet.id_ref_expr(binding_name.as_str(), ident.span);
            return;
          }
        }
      }
    }

    self.rewrite_import_meta_hot(it);

    walk_mut::walk_expression(self, it);
  }

  fn visit_call_expression(&mut self, call_expr: &mut ast::CallExpression<'ast>) {
    self.rewrite_hot_accept_call_deps(call_expr);
    walk_mut::walk_call_expression(self, call_expr);
  }
}
