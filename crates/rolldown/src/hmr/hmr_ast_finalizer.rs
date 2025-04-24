use oxc::{
  allocator::{Allocator, Box as ArenaBox, Dummy, IntoIn, TakeIn},
  ast::NONE,
  ast::ast::{self, ExportDefaultDeclarationKind},
  ast_visit::{VisitMut, walk_mut},
  semantic::{Scoping, SymbolId},
  span::{Atom, SPAN},
};

use rolldown_common::{IndexModules, Module, ModuleIdx, NormalModule};
use rolldown_ecmascript_utils::{
  AstSnippet, BindingIdentifierExt, BindingPatternExt, ExpressionExt, quote_stmt, quote_stmts,
};
use rolldown_utils::{ecmascript::is_validate_identifier_name, indexmap::FxIndexSet};
use rustc_hash::FxHashMap;

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
  pub exports: FxHashMap<Atom<'ast>, Atom<'ast>>,
  pub dependencies: FxIndexSet<ModuleIdx>,
}

impl<'ast> HmrAstFinalizer<'_, 'ast> {
  pub fn generate_hmr_header(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];

    // `import.meta.hot = __rolldown_runtime__.createModuleHotContext(moduleId);`
    ret.push(self.generate_stmt_of_init_module_hot_context());

    ret.extend(self.generate_runtime_module_register_for_hmr());

    ret
  }

  pub fn generate_stmt_of_init_module_hot_context(&self) -> ast::Statement<'ast> {
    // import.meta.hot = __rolldown_runtime__.createModuleHotContext(moduleId);
    let stmt = quote_stmt(
      self.alloc,
      &format!(
        "import.meta.hot = __rolldown_runtime__.createModuleHotContext({:?});",
        self.module.stable_id
      ),
    );
    stmt
  }

  pub fn generate_runtime_module_register_for_hmr(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];

    let module_exports = match self.module.exports_kind {
      rolldown_common::ExportsKind::Esm => {
        // TODO: Still we could reuse use module namespace def

        // Empty object `{}`
        let mut arg_obj_expr =
          self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec());

        self.exports.iter().for_each(|(exported, local_name)| {
          // prop_name: () => returned
          let prop_name = exported;
          let returned = self.snippet.id_ref_expr(local_name, SPAN);
          arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
            ast::ObjectProperty {
              key: if is_validate_identifier_name(prop_name) {
                ast::PropertyKey::StaticIdentifier(
                  self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
                )
              } else {
                ast::PropertyKey::StringLiteral(self.snippet.alloc_string_literal(prop_name, SPAN))
              },
              value: self.snippet.only_return_arrow_expr(returned),
              ..ast::ObjectProperty::dummy(self.alloc)
            }
            .into_in(self.alloc),
          ));
        });
        ast::Argument::ObjectExpression(arg_obj_expr)
      }
      rolldown_common::ExportsKind::CommonJs => {
        // `module.exports`
        ast::Argument::StaticMemberExpression(self.snippet.builder.alloc_static_member_expression(
          SPAN,
          self.snippet.id_ref_expr("module", SPAN),
          self.snippet.id_name("exports", SPAN),
          false,
        ))
      }
      rolldown_common::ExportsKind::None => ast::Argument::ObjectExpression(
        // `{}`
        self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec()),
      ),
    };

    // __rolldown_runtime__.register(moduleId, module)
    let mut arguments = self.snippet.builder.vec_from_array([
      ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
        SPAN,
        &self.module.stable_id,
        None,
      )),
      module_exports,
    ]);

    if self.module.exports_kind.is_commonjs() {
      // __rolldown_runtime__.register(moduleId, module, { cjs: true })
      arguments.push(ast::Argument::ObjectExpression(
        self.snippet.builder.alloc_object_expression(
          SPAN,
          self.snippet.builder.vec1(ast::ObjectPropertyKind::ObjectProperty(
            ast::ObjectProperty {
              key: ast::PropertyKey::StaticIdentifier(
                self.snippet.id_name("cjs", SPAN).into_in(self.alloc),
              ),
              value: ast::Expression::BooleanLiteral(
                self.snippet.builder.alloc_boolean_literal(SPAN, true),
              ),
              ..ast::ObjectProperty::dummy(self.alloc)
            }
            .into_in(self.alloc),
          )),
        ),
      ));
    }

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

    let dev_runtime_head = self.generate_hmr_header();

    try_block
      .body
      .reserve_exact(dev_runtime_head.len() + it.body.len() + dependencies_init_fn_stmts.len());
    try_block.body.extend(dev_runtime_head);
    try_block.body.extend(dependencies_init_fn_stmts);
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
      self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
        SPAN,
        ast::VariableDeclarationKind::Var,
        self.snippet.builder.binding_pattern(
          ast::BindingPatternKind::BindingIdentifier(
            self.snippet.builder.alloc_binding_identifier(SPAN, init_fn_name),
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
      )),
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
          match &self.modules[rec.resolved_module] {
            Module::Normal(importee) => {
              self.dependencies.insert(rec.resolved_module);
              let id = &importee.stable_id;
              let binding_name = format!("import_{}", importee.repr_name);
              let stmt = quote_stmt(
                self.alloc,
                format!("const {binding_name} = __rolldown_runtime__.loadExports({id:?});",)
                  .as_str(),
              );
              import_decl.specifiers.as_ref().inspect(|specifiers| {
                specifiers.iter().for_each(|spec| match spec {
                  ast::ImportDeclarationSpecifier::ImportSpecifier(import_specifier) => {
                    self.import_binding.insert(
                      import_specifier.local.expect_symbol_id(),
                      format!("{binding_name}.{}", import_specifier.imported.name()),
                    );
                  }
                  ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(
                    import_default_specifier,
                  ) => {
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
              *node = stmt;
            }
            Module::External(_importee) => {
              todo!("handle external module");
            }
          }
        }
        ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
          if let Some(_source) = &decl.source {
            // TODO: support reexport
            // export {} from '...'
            decl.specifiers.iter().for_each(|spec| {
              self.exports.insert(spec.exported.name(), spec.local.name());
            });
          } else if let Some(decl) = &mut decl.declaration {
            match decl {
              ast::Declaration::VariableDeclaration(var_decl) => {
                // export var foo = 1
                // export var { foo, bar } = { foo: 1, bar: 2 }
                var_decl.declarations.iter().for_each(|decl| {
                  decl.id.binding_identifiers().into_iter().for_each(|id| {
                    self.exports.insert(id.name, id.name);
                  });
                });
              }
              ast::Declaration::FunctionDeclaration(fn_decl) => {
                // export function foo() {}
                let id = fn_decl.id.as_ref().unwrap();
                self.exports.insert(id.name, id.name);
              }
              ast::Declaration::ClassDeclaration(cls_decl) => {
                // export class Foo {}
                let id = cls_decl.id.as_ref().unwrap();
                self.exports.insert(id.name, id.name);
              }
              _ => unreachable!("doesn't support ts now"),
            }
            *node = ast::Statement::from(decl.take_in(self.alloc));
          } else {
            // export { foo, bar as bar2 }
            decl.specifiers.iter().for_each(|spec| {
              self.exports.insert(spec.exported.name(), spec.local.name());
            });
            *node =
              ast::Statement::EmptyStatement(self.snippet.builder.alloc_empty_statement(SPAN));
          }
        }
        ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => match &mut decl.declaration {
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
              self.exports.insert("default".into(), id.name);
            } else {
              function.id = Some(self.snippet.id("__rolldown_default__", SPAN));
              self.exports.insert("default".into(), "__rolldown_default__".into());
            }
            *node = ast::Statement::FunctionDeclaration(ArenaBox::new_in(
              function.as_mut().take_in(self.alloc),
              self.alloc,
            ));
          }
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
              self.exports.insert("default".into(), id.name);
            } else {
              class.id = Some(self.snippet.id("__rolldown_default__", SPAN));
              self.exports.insert("default".into(), "__rolldown_default__".into());
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
            self.exports.insert("default".into(), "__rolldown_default__".into());
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

    walk_mut::walk_expression(self, it);
  }
}
