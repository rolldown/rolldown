use oxc::allocator::GetAllocator;
use oxc::ast::ast::Str;
use oxc::{
  allocator::IntoIn,
  ast::{
    ast::{
      self, BindingIdentifier, ExportDefaultDeclarationKind, Expression, IdentifierName,
      ObjectPropertyKind, Statement,
    },
    builder::NONE,
  },
  semantic::{IsGlobalReference, Scoping, SymbolId},
  span::{SPAN, Span},
};

use oxc::ast::builder::AstBuilder;
use rolldown_common::{
  ExternalModule, ImportRecordIdx, ImportRecordMeta, IndexModules, Module, ModuleIdx, NormalModule,
};
use rolldown_ecmascript::CJS_REQUIRE_REF_STR;
use rolldown_ecmascript_utils::{
  BindingIdentifierFactoryExt as _, ExpressionExt, ExpressionFactoryExt as _,
  IdentifierNameFactoryExt as _, ObjectPropertyKindFactoryExt as _, StatementFactoryExt as _,
};
use rolldown_utils::{
  ecmascript::is_validate_identifier_name,
  indexmap::{FxIndexMap, FxIndexSet},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::hmr::utils::{HmrAstBuilder, MODULE_EXPORTS_NAME_FOR_ESM};

pub struct HmrAstFinalizer<'me, 'ast> {
  // Outside input
  pub ast_builder: AstBuilder<'ast>,
  pub modules: &'me IndexModules,
  pub module: &'me NormalModule,
  pub use_pife_for_module_wrappers: bool,

  // Each module has a unique index, which is used to generate something that needs to be unique.
  pub unique_index: usize,

  // --- Internal state
  /// For
  ///
  /// ```js
  /// import foo, { bar } from './foo.js';
  /// ```
  ///
  /// , assuming `foo` and `bar` have symbol id `1` and `2`, we will store mapping like:
  ///
  /// - 1 => "import_foo_1.default"
  /// - 2 => "import_foo_1.bar"
  ///
  /// `import_foo_1` is the binding name we gonna used to generate code like
  ///
  /// ```js
  /// const import_foo_1 = __rolldown_runtime__.loadExports('./foo.js');
  /// ```
  pub import_bindings: FxHashMap<SymbolId, String>,
  pub exports: oxc::allocator::Vec<'ast, ObjectPropertyKind<'ast>>,
  pub re_export_all_dependencies: FxIndexSet<ModuleIdx>,
  pub dependencies: FxIndexSet<ModuleIdx>,
  pub imports: FxHashSet<ModuleIdx>,
  pub generated_static_import_infos: FxHashMap<ModuleIdx, String>,
  // We need to store the static import statements for external separately, so we could put them outside of the `try` block.
  pub generated_static_import_stmts_from_external: FxIndexMap<ModuleIdx, ast::Statement<'ast>>,
  pub named_exports: FxHashMap<Str<'ast>, NamedExport>,
}

impl<'ast> HmrAstFinalizer<'_, 'ast> {
  #[expect(clippy::too_many_lines)]
  pub fn handle_top_level_stmt(
    &mut self,
    program_body: &mut oxc::allocator::Vec<'ast, ast::Statement<'ast>>,
    node: ast::Statement<'ast>,
    scoping: &Scoping,
  ) {
    match node {
      ast::Statement::ImportDeclaration(import_decl) => {
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
        let rec_id = self.module.imports[&import_decl.node_id()];
        let rec = &self.module.import_records[rec_id];
        let Some(importee_idx) = rec.resolved_module else { return };
        let importee = &self.modules[importee_idx];
        self.dependencies.insert(importee_idx);
        let binding_name = self.ensure_static_import_info(importee_idx, rec_id).to_string();
        import_decl.specifiers.as_ref().inspect(|specifiers| {
          specifiers.iter().for_each(|spec| match spec {
            ast::ImportDeclarationSpecifier::ImportSpecifier(import_specifier) => {
              self.import_bindings.insert(
                import_specifier.local.symbol_id(),
                format!("{binding_name}.{}", import_specifier.imported.name()),
              );
            }
            ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(import_default_specifier) => {
              self.import_bindings.insert(
                import_default_specifier.local.symbol_id(),
                format!("{binding_name}.default"),
              );
            }
            ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
              import_namespace_specifier,
            ) => {
              self
                .import_bindings
                .insert(import_namespace_specifier.local.symbol_id(), binding_name.clone());
            }
          });
        });
        match importee {
          Module::Normal(_) => {
            if let Some(stmt) =
              self.create_load_exports_call_stmt(importee, &binding_name, import_decl.span)
            {
              program_body.push(stmt);
            }
          }
          Module::External(importee_ext) => {
            self.create_static_import_stmt_from_external_module(
              importee_ext,
              &binding_name,
              import_decl.span,
            );
          }
        }
      }
      ast::Statement::ExportNamedDeclaration(mut decl) => {
        if decl.source.is_some() {
          // export {} from '...'
          let rec_id = self.module.imports[&decl.node_id()];
          let rec = &self.module.import_records[rec_id];
          let Some(importee_idx) = rec.resolved_module else { return };
          let importee = &self.modules[importee_idx];
          self.dependencies.insert(importee_idx);
          let binding_name = self.ensure_static_import_info(importee_idx, rec_id).to_string();
          self.exports.extend(decl.specifiers.iter().map(|specifier| {
            ObjectPropertyKind::new_lazy_export_property(
              &specifier.exported.name(),
              match &specifier.local {
                ast::ModuleExportName::IdentifierName(ident) => {
                  Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
                    SPAN,
                    Expression::new_id_ref_expr(SPAN, &binding_name, &self.ast_builder),
                    ast::IdentifierName::new(SPAN, ident.name.as_str(), &self.ast_builder),
                    false,
                    &self.ast_builder,
                  ))
                }
                ast::ModuleExportName::StringLiteral(str) => {
                  Expression::ComputedMemberExpression(ast::ComputedMemberExpression::boxed(
                    SPAN,
                    Expression::new_id_ref_expr(SPAN, &binding_name, &self.ast_builder),
                    ast::Expression::new_string_literal(
                      SPAN,
                      str.value.as_str(),
                      None,
                      &self.ast_builder,
                    ),
                    false,
                    &self.ast_builder,
                  ))
                }
                ast::ModuleExportName::IdentifierReference(_) => {
                  unreachable!(
                    "ModuleExportName IdentifierReference is invalid in ExportNamedDeclaration with source"
                  )
                }
              },
              matches!(specifier.exported, ast::ModuleExportName::StringLiteral(_)), &self.ast_builder
            )
          }));
          if let Some(stmt) = self.create_load_exports_call_stmt(importee, &binding_name, decl.span)
          {
            program_body.push(stmt);
          }
        } else if let Some(declaration) = decl.declaration.take() {
          match &declaration {
            ast::Declaration::VariableDeclaration(var_decl) => {
              // export var foo = 1
              // export var { foo, bar } = { foo: 1, bar: 2 }
              self.exports.extend(var_decl.declarations.iter().flat_map(|decl| {
                decl.id.get_binding_identifiers().into_iter().map(|ident| {
                  ObjectPropertyKind::new_lazy_export_property(
                    ident.name.as_str(),
                    Expression::new_id_ref_expr(SPAN, ident.name.as_str(), &self.ast_builder),
                    false,
                    &self.ast_builder,
                  )
                })
              }));
            }
            ast::Declaration::FunctionDeclaration(fn_decl) => {
              // export function foo() {}
              let id = fn_decl.id.as_ref().unwrap().name.as_str();
              self.exports.push(ObjectPropertyKind::new_lazy_export_property(
                id,
                Expression::new_id_ref_expr(SPAN, id, &self.ast_builder),
                false,
                &self.ast_builder,
              ));
            }
            ast::Declaration::ClassDeclaration(cls_decl) => {
              // export class Foo {}
              let id = cls_decl.id.as_ref().unwrap().name.as_str();
              self.exports.push(ObjectPropertyKind::new_lazy_export_property(
                id,
                Expression::new_id_ref_expr(SPAN, id, &self.ast_builder),
                false,
                &self.ast_builder,
              ));
            }
            _ => unreachable!("doesn't support ts now"),
          }
          program_body.push(ast::Statement::from(declaration));
        } else {
          // export { foo, bar as bar2 }
          decl.specifiers.iter().for_each(|specifier| {
            if let Some(symbol_id) = scoping.get_root_binding(specifier.local.name().into()) {
              self
                .named_exports
                .insert(specifier.exported.name(), NamedExport { local_binding: symbol_id });
            } else {
              // TODO: export undefined variable
            }
          });
        }
      }
      ast::Statement::ExportDefaultDeclaration(decl) => match decl.unbox().declaration {
        ast::ExportDefaultDeclarationKind::FunctionDeclaration(mut function) => {
          if let Some(id) = &function.id {
            self.exports.push(ObjectPropertyKind::new_lazy_export_property(
              "default",
              Expression::new_id_ref_expr(SPAN, &id.name, &self.ast_builder),
              false,
              &self.ast_builder,
            ));
          } else {
            function.id =
              Some(BindingIdentifier::new_id(SPAN, "__rolldown_default__", &self.ast_builder));
            self.exports.push(ObjectPropertyKind::new_lazy_export_property(
              "default",
              Expression::new_id_ref_expr(SPAN, "__rolldown_default__", &self.ast_builder),
              false,
              &self.ast_builder,
            ));
          }
          program_body.push(ast::Statement::FunctionDeclaration(function));
        }
        ast::ExportDefaultDeclarationKind::ClassDeclaration(mut class) => {
          if let Some(id) = &class.id {
            self.exports.push(ObjectPropertyKind::new_lazy_export_property(
              "default",
              Expression::new_id_ref_expr(SPAN, &id.name, &self.ast_builder),
              false,
              &self.ast_builder,
            ));
          } else {
            class.id =
              Some(BindingIdentifier::new_id(SPAN, "__rolldown_default__", &self.ast_builder));
            self.exports.push(ObjectPropertyKind::new_lazy_export_property(
              "default",
              Expression::new_id_ref_expr(SPAN, "__rolldown_default__", &self.ast_builder),
              false,
              &self.ast_builder,
            ));
          }
          program_body.push(ast::Statement::ClassDeclaration(class));
        }
        expr @ ast::match_expression!(ExportDefaultDeclarationKind) => {
          let expr = expr.into_expression();
          // Transform `export default [expression]` => `var __rolldown_default__ = [expression]`
          program_body.push(Statement::new_var_decl(
            "__rolldown_default__",
            expr,
            &self.ast_builder,
          ));
          self.exports.push(ObjectPropertyKind::new_lazy_export_property(
            "default",
            Expression::new_id_ref_expr(SPAN, "__rolldown_default__", &self.ast_builder),
            false,
            &self.ast_builder,
          ));
        }
        unhandled_kind => {
          unreachable!("Unexpected export default declaration kind: {unhandled_kind:#?}");
        }
      },
      ast::Statement::ExportAllDeclaration(export_all_decl) => {
        let rec_id = self.module.imports[&export_all_decl.node_id()];
        let rec = &self.module.import_records[rec_id];
        let Some(importee_idx) = rec.resolved_module else { return };
        let importee = &self.modules[importee_idx];
        self.dependencies.insert(importee_idx);
        let binding_name = self.ensure_static_import_info(importee_idx, rec_id).to_string();
        if let Some(stmt) =
          self.create_load_exports_call_stmt(importee, &binding_name, export_all_decl.span)
        {
          program_body.push(stmt);
        }
        if let Some(stmt) =
          self.create_re_export_call_stmt(importee, &binding_name, export_all_decl.span)
        {
          program_body.push(stmt);
        }
      }
      // Every other statement is kept as-is. That's ordinary statements, which need no
      // rewriting, and also the TypeScript module declarations (`export =`, `export as
      // namespace`), which rolldown should have pre-processed away already - if one reaches
      // here something went wrong upstream, and keeping it beats panicking.
      node => {
        program_body.push(node);
      }
    }
  }

  pub fn ensure_static_import_info(
    &mut self,
    importee_idx: ModuleIdx,
    rec_id: ImportRecordIdx,
  ) -> &str {
    self.generated_static_import_infos.entry(importee_idx).or_insert_with(|| {
      let importee = &self.modules[importee_idx];

      format!("import_{}_{}{}", importee.repr_name(), self.unique_index, rec_id.raw())
    })
  }

  pub fn module_exports_name(&self) -> &'static str {
    if self.module.exports_kind.is_commonjs() {
      "module.exports"
    } else {
      MODULE_EXPORTS_NAME_FOR_ESM
    }
  }

  pub fn generate_runtime_module_register_for_hmr(
    &mut self,
    scoping: &Scoping,
  ) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if self.module.exports_kind == rolldown_common::ExportsKind::Esm {
      let binding_name_for_namespace_object_ref = self.module_exports_name();

      ret.extend(self.generate_declaration_of_module_namespace_object(
        binding_name_for_namespace_object_ref,
        scoping,
      ));
    }
    ret.push(self.create_register_module_stmt());

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
        let Some(module_idx) = import_record.resolved_module else { return };
        // Use stable module ID for consistent runtime lookup
        string_literal.value =
          Str::from_str_in(self.modules[module_idx].stable_id(), &self.ast_builder);
      }
      ast::Argument::ArrayExpression(array_expression) => {
        // `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`
        array_expression.elements.iter_mut().for_each(|element| {
          if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
            let import_record =
              &self.module.import_records[self.module.hmr_info.module_request_to_import_record_idx
                [string_literal.value.as_str()]];
            let Some(module_idx) = import_record.resolved_module else { return };
            // Use stable module ID for consistent runtime lookup
            string_literal.value =
              Str::from_str_in(self.modules[module_idx].stable_id(), &self.ast_builder);
          }
        });
      }
      _ => {}
    }
  }

  pub fn rewrite_import_meta_hot(&self, expr: &mut ast::Expression<'ast>) {
    if expr.is_import_meta_hot() {
      let hot_name = format!("hot_{}", self.module.repr_name);
      *expr = Expression::new_id_ref_expr(SPAN, &hot_name, &self.ast_builder);
    }
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

    // Use stable module ID for consistent runtime lookup
    let id = importee.stable_id();
    let interop = match importee {
      Module::Normal(importee) => self.module.interop(importee),
      Module::External(_) => None,
    };
    let call_expr = Expression::new_call_with_arg(
      Expression::new_member_access_expr("__rolldown_runtime__", "loadExports", &self.ast_builder),
      ast::Expression::new_string_literal(
        SPAN,
        Str::from_str_in(id, &self.ast_builder),
        None,
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    );

    // var [binding_name] = [__toESM-wrapped loadExports call];
    let stmt = Statement::from(ast::Declaration::new_variable_declaration(
      span,
      ast::VariableDeclarationKind::Var,
      oxc::allocator::Vec::from_value_in(
        ast::VariableDeclarator::new(
          SPAN,
          ast::VariableDeclarationKind::Var,
          ast::BindingPattern::new_binding_identifier(
            SPAN,
            Str::from_str_in(binding_name, &self.ast_builder),
            &self.ast_builder,
          ),
          NONE,
          Some(Expression::new_to_esm_call_with_interop(
            "__rolldown_runtime__.__toESM",
            call_expr,
            interop,
            &self.ast_builder,
          )),
          false,
          &self.ast_builder,
        ),
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    ));
    Some(stmt)
  }

  fn create_static_import_stmt_from_external_module(
    &mut self,
    importee: &ExternalModule,
    binding_name: &str,
    _span: Span,
  ) {
    if self.generated_static_import_stmts_from_external.contains_key(&importee.idx) {
      return;
    }

    let module_request = &importee.id;

    // import * as [binding_name] from 'external';
    let stmt = Statement::new_import_star_stmt(module_request, binding_name, &self.ast_builder);

    self.generated_static_import_stmts_from_external.insert(importee.idx, stmt);
  }

  fn create_re_export_call_stmt(
    &mut self,
    importee: &Module,
    binding_name: &str,
    span: Span,
  ) -> Option<Statement<'ast>> {
    if self.re_export_all_dependencies.contains(&importee.idx()) {
      return None;
    }
    self.re_export_all_dependencies.insert(importee.idx());

    let self_exports = self.module_exports_name();

    let call_expr = ast::Expression::new_call_expression(
      SPAN,
      Expression::new_member_access_expr("__rolldown_runtime__", "__reExport", &self.ast_builder),
      NONE,
      oxc::allocator::Vec::from_iter_in(
        [
          ast::Argument::from(Expression::new_id_ref_expr(SPAN, self_exports, &self.ast_builder)),
          ast::Argument::from(Expression::new_id_ref_expr(SPAN, binding_name, &self.ast_builder)),
        ],
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    );

    Some(ast::Statement::ExpressionStatement(ast::ExpressionStatement::boxed(
      span,
      call_expr,
      &self.ast_builder,
    )))
  }

  fn generate_declaration_of_module_namespace_object(
    &mut self,
    binding_name_for_namespace_object_ref: &str,
    scoping: &Scoping,
  ) -> Vec<ast::Statement<'ast>> {
    // TODO reexport external module

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::boxed(
      SPAN,
      oxc::allocator::Vec::with_capacity_in(self.exports.len(), &self.ast_builder),
      &self.ast_builder,
    );
    arg_obj_expr.properties.extend(self.exports.drain(..));
    arg_obj_expr.properties.extend(self.named_exports.iter().map(|(exported, named_export)| {
      let expr = if let Some(local_binding) = self.import_bindings.get(&named_export.local_binding)
      {
        Expression::new_id_ref_expr(SPAN, local_binding, &self.ast_builder)
      } else {
        let name = scoping.symbol_name(named_export.local_binding);
        Expression::new_id_ref_expr(SPAN, name, &self.ast_builder)
      };
      // Use computed property syntax for non-identifier export names (e.g., 'rolldown:exports')
      let computed = !is_validate_identifier_name(exported.as_str());
      ObjectPropertyKind::new_lazy_export_property(exported, expr, computed, &self.ast_builder)
    }));

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let export_call_expr = ast::Expression::new_call_expression(
      SPAN,
      Expression::new_id_ref_expr(SPAN, "__rolldown_runtime__.__exportAll", &self.ast_builder),
      NONE,
      oxc::allocator::Vec::from_array_in(
        [ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.ast_builder.allocator()))],
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    );

    // construct `var [binding_name_for_namespace_object_ref] = __exportAll({ prop_name: () => returned, ... })`
    let decl_stmt = Statement::new_var_decl(
      binding_name_for_namespace_object_ref,
      export_call_expr,
      &self.ast_builder,
    );
    vec![decl_stmt]
  }

  // Rewrite `import(...)` to sensible form.
  pub fn try_rewrite_dynamic_import(&self, it: &mut ast::Expression<'ast>) {
    let ast::Expression::ImportExpression(import_expr) = it else {
      return;
    };

    let Some(rec_idx) = self.module.imports.get(&import_expr.node_id()) else {
      return;
    };

    let Some(importee_idx) = self.module.import_records[*rec_idx].resolved_module else {
      return;
    };

    let Module::Normal(importee) = &self.modules[importee_idx] else {
      // Not a normal module, skip
      return;
    };

    // Handle lazy proxy modules - rewrite to mirror the proxy module's runtime contract.
    //
    // In a regular full build, scope finalizer rewrites `import('./foo')` to point at the
    // proxy module's chunk URL. That chunk's content is `proxy-module-template.js`, which
    // exposes `'rolldown:exports'` at the top level so consumers can do
    // `.then(__unwrap_lazy_compilation_entry).then(m => m.X)`.
    //
    // In HMR partial bundles there's no separately bundled proxy chunk - the proxy module's
    // body gets wrapped inside a `createEsmInitializer` and its top-level `export` is lost.
    // To keep the same surface as the full build, we rewrite the dynamic import to:
    //
    //   import(`/@vite/lazy?id=...&clientId=...`)
    //     .then(() => __rolldown_runtime__.loadExports("<stable_proxy_id>"))
    //
    // After the partial bundle evaluates, the proxy module is registered under
    // `<stable_proxy_id>` with a `'rolldown:exports'` getter (set up by `__exportAll` inside
    // the init wrapper). Reading it back via `loadExports` yields the namespace object that
    // the existing `__unwrap_lazy_compilation_entry` chain expects.
    //
    // TODO: hyf0 should switch to a more robust way to identify lazy proxy modules
    if importee.id.contains("?rolldown-lazy=1") {
      // Build: encodeURIComponent(importee.id)
      let encode_call = ast::Expression::CallExpression(ast::CallExpression::boxed(
        SPAN,
        Expression::new_id_ref_expr(SPAN, "encodeURIComponent", &self.ast_builder),
        NONE,
        oxc::allocator::Vec::from_value_in(
          ast::Argument::StringLiteral(ast::StringLiteral::boxed(
            SPAN,
            Str::from_str_in(&importee.id, &self.ast_builder),
            None,
            &self.ast_builder,
          )),
          &self.ast_builder,
        ),
        false,
        &self.ast_builder,
      ));

      // Build template literal: `/@vite/lazy?id=${encodeURIComponent(importee.id)}&clientId=${__rolldown_runtime__.clientId}`
      let url_expr = {
        let quasis = oxc::allocator::Vec::from_iter_in(
          [
            ast::TemplateElement::new(
              SPAN,
              ast::TemplateElementValue {
                raw: Str::from_str_in("/@vite/lazy?id=", &self.ast_builder),
                cooked: None,
              },
              false,
              &self.ast_builder,
            ),
            ast::TemplateElement::new(
              SPAN,
              ast::TemplateElementValue {
                raw: Str::from_str_in("&clientId=", &self.ast_builder),
                cooked: None,
              },
              false,
              &self.ast_builder,
            ),
            ast::TemplateElement::new(
              SPAN,
              ast::TemplateElementValue {
                raw: Str::from_str_in("", &self.ast_builder),
                cooked: None,
              },
              true,
              &self.ast_builder,
            ),
          ],
          &self.ast_builder,
        );
        let expressions = oxc::allocator::Vec::from_iter_in(
          [
            encode_call,
            Expression::new_member_access_expr(
              "__rolldown_runtime__",
              "clientId",
              &self.ast_builder,
            ),
          ],
          &self.ast_builder,
        );
        ast::Expression::new_template_literal(SPAN, quasis, expressions, &self.ast_builder)
      };

      // Build: import(`/@vite/lazy?id=...&clientId=...`)
      let import_expr =
        ast::Expression::new_import_expression(SPAN, url_expr, None, None, &self.ast_builder);

      // Build: __rolldown_runtime__.loadExports("<stable_proxy_id>")
      let load_exports_call = ast::Expression::CallExpression(ast::CallExpression::boxed(
        SPAN,
        Expression::new_id_ref_expr(SPAN, "__rolldown_runtime__.loadExports", &self.ast_builder),
        NONE,
        oxc::allocator::Vec::from_value_in(
          ast::Argument::StringLiteral(ast::StringLiteral::boxed(
            SPAN,
            Str::from_str_in(&importee.stable_id, &self.ast_builder),
            None,
            &self.ast_builder,
          )),
          &self.ast_builder,
        ),
        false,
        &self.ast_builder,
      ));

      // Build: () => __rolldown_runtime__.loadExports("<stable_proxy_id>")
      let arrow_fn = ast::Expression::new_arrow_function_expression(
        SPAN,
        /* expression */ true,
        /* async */ false,
        NONE,
        ast::FormalParameters::new(
          SPAN,
          ast::FormalParameterKind::ArrowFormalParameters,
          oxc::allocator::Vec::new_in(&self.ast_builder),
          NONE,
          &self.ast_builder,
        ),
        NONE,
        ast::FunctionBody::new(
          SPAN,
          oxc::allocator::Vec::new_in(&self.ast_builder),
          oxc::allocator::Vec::from_value_in(
            ast::Statement::ExpressionStatement(ast::ExpressionStatement::boxed(
              SPAN,
              load_exports_call,
              &self.ast_builder,
            )),
            &self.ast_builder,
          ),
          &self.ast_builder,
        ),
        &self.ast_builder,
      );

      // Build: import(...).then(() => __rolldown_runtime__.loadExports("..."))
      let then_callee = Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
        SPAN,
        import_expr,
        ast::IdentifierName::new(SPAN, "then", &self.ast_builder),
        false,
        &self.ast_builder,
      ));

      *it = ast::Expression::new_call_expression(
        SPAN,
        then_callee,
        NONE,
        oxc::allocator::Vec::from_value_in(ast::Argument::from(arrow_fn), &self.ast_builder),
        false,
        &self.ast_builder,
      );
      return;
    }

    // FIXME: consider about CommonJS interop
    let is_importee_cjs = importee.exports_kind == rolldown_common::ExportsKind::CommonJs;

    // __rolldown_runtime__.loadExports('./foo.js')
    // Use stable module ID for consistent runtime lookup
    let mut load_exports_call_expr = ast::Expression::CallExpression(ast::CallExpression::boxed(
      SPAN,
      Expression::new_id_ref_expr(SPAN, "__rolldown_runtime__.loadExports", &self.ast_builder),
      NONE,
      oxc::allocator::Vec::from_value_in(
        ast::Argument::StringLiteral(ast::StringLiteral::boxed(
          SPAN,
          Str::from_str_in(&importee.stable_id, &self.ast_builder),
          None,
          &self.ast_builder,
        )),
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    ));

    if is_importee_cjs {
      let is_node_cjs = importee.def_format.is_commonjs();

      let mut args = oxc::allocator::Vec::from_value_in(
        ast::Argument::from(load_exports_call_expr),
        &self.ast_builder,
      );
      if is_node_cjs {
        args.push(ast::Argument::from(ast::Expression::new_numeric_literal(
          SPAN,
          1.0,
          None,
          ast::NumberBase::Decimal,
          &self.ast_builder,
        )));
      }

      // __rolldown_runtime__.__toDynamicImportESM(__rolldown_runtime__.loadExports('./foo.js'), node_mode)
      load_exports_call_expr = ast::Expression::new_call_expression(
        SPAN,
        Expression::new_id_ref_expr(
          SPAN,
          "__rolldown_runtime__.__toDynamicImportESM",
          &self.ast_builder,
        ),
        NONE,
        args,
        false,
        &self.ast_builder,
      );
    }

    // `import()` is an execution point like static imports and `require`, so it goes through
    // the same unconditional registry gate. Factories outlive the payload that shipped them:
    // whether the importee rides this payload says nothing about whether it will be registered
    // when these bytes run (a later patch may evict it), so the emitted code must not depend
    // on payload membership. `initModule` short-circuits on a resident module and re-runs an
    // evicted one from its factory.
    // Turn `import('./foo.js')` into
    // `(__rolldown_runtime__.initModule('./foo.js'), Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js')))`
    let init_call = self.make_init_module_call(&self.modules[importee_idx]);
    let promise_resolve_then_load_exports =
      Expression::new_promise_resolve_then(load_exports_call_expr, &self.ast_builder);
    *it = ast::Expression::SequenceExpression(ast::SequenceExpression::boxed(
      SPAN,
      oxc::allocator::Vec::from_array_in(
        [init_call, promise_resolve_then_load_exports],
        &self.ast_builder,
      ),
      &self.ast_builder,
    ));
  }

  pub fn try_rewrite_require(
    &self,
    it: &mut ast::Expression<'ast>,
    ctx: &oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let scoping = ctx.scoping();

    // Rewrite standalone `require` to `__rolldown_runtime__.loadExports`
    if let Some(id_ref) = it.as_identifier()
      && id_ref.name == CJS_REQUIRE_REF_STR
      && id_ref.is_global_reference(scoping)
      && !ctx.parent().is_call_expression()
    {
      *it = Expression::new_member_access_expr(
        "__rolldown_runtime__",
        "loadExports",
        &self.ast_builder,
      );
    }

    // Rewrite `require(...)` to `(require_xxx(), __rolldown_runtime__.loadExports())` or keep it as is for external module importee.
    let ast::Expression::CallExpression(call_expr) = it else {
      return;
    };

    if !call_expr
      .callee
      .as_identifier()
      .is_some_and(|id| id.name == CJS_REQUIRE_REF_STR && id.is_global_reference(scoping))
    {
      return;
    }

    let Some(rec_idx) = self.module.imports.get(&call_expr.node_id()) else {
      return;
    };

    let rec = &self.module.import_records[*rec_idx];
    let Some(importee_idx) = rec.resolved_module else {
      return;
    };

    let Module::Normal(importee) = &self.modules[importee_idx] else {
      // Not a normal module, skip
      return;
    };

    let is_importee_cjs = importee.exports_kind == rolldown_common::ExportsKind::CommonJs;

    // Use stable module ID for consistent runtime lookup
    let load_exports_call = Expression::new_call_with_arg(
      Expression::new_member_access_expr("__rolldown_runtime__", "loadExports", &self.ast_builder),
      ast::Expression::new_string_literal(
        SPAN,
        Str::from_str_in(&importee.stable_id, &self.ast_builder),
        None,
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    );

    let load_exports_expr = if importee.meta.has_lazy_export() || is_importee_cjs {
      // Note that HMR finalizer is only able to see scanner-level exports_kind, this means that the result
      // from `determine_module_exports_kind` is not available here. So we have to use some heuristics to determine
      // whether the importee is CommonJS or has lazy export, and handle them in a special way.
      //
      // 1. For the case of `is_importee_cjs`,
      // the runtime will always have `module.exports`. This is determined in `determine_module_exports_kind`.
      //
      // 2. For the case of `has_lazy_export`,
      // here we're inside `try_rewrite_require`, which means the original code is `require(...)`.
      //
      // Modules that have lazy export is of these `ModuleType`: `Json`, `Text`, `Base64`, `Dataurl`.
      // These data type does not have `export`, `module.exports` or any export keyword at runtime,
      // so they're `ExportsKind::None` by default.
      //
      // For those lazy export modules, if `ImportKind` is `Require`, which is the case here,
      // and the importee has `ExportsKind::None`, then the importee's `WrapKind` is set to `WrapKind::Cjs`.
      // So here we know for sure that the importee is using `module.exports` at runtime.
      // So `loadExports(id)` returns the value directly.
      //
      // This is a way to mimic the same mechanism of `determine_module_exports_kind`.
      //
      // TODO(hana): we should think about a more robust way to track the consolidated export type of a module in the future.
      // Listing all the special cases like this is error-prone.
      load_exports_call
    } else if rec.meta.contains(ImportRecordMeta::JsonModule) {
      // Vite-mode JSON: ESM-wrapped at runtime, unwrap to the JSON value.
      let to_commonjs_call = Expression::new_call_with_arg(
        Expression::new_member_access_expr(
          "__rolldown_runtime__",
          "__toCommonJS",
          &self.ast_builder,
        ),
        load_exports_call,
        false,
        &self.ast_builder,
      );
      Expression::from(ast::MemberExpression::new_static_member_expression(
        SPAN,
        to_commonjs_call,
        IdentifierName::new_id_name(SPAN, "default", &self.ast_builder),
        false,
        &self.ast_builder,
      ))
    } else {
      Expression::new_call_with_arg(
        Expression::new_member_access_expr(
          "__rolldown_runtime__",
          "__toCommonJS",
          &self.ast_builder,
        ),
        load_exports_call,
        false,
        &self.ast_builder,
      )
    };

    // `require` is a static record and an execution point: init through the one
    // registry gate — a resident module short-circuits, a carried factory runs.
    // Turn `require('./foo.js')` into
    // `(__rolldown_runtime__.initModule('./foo.js'), __rolldown_runtime__.loadExports('./foo.js'))`
    *it = Expression::new_seq_in_parens(
      self.make_init_module_call(&self.modules[importee_idx]),
      load_exports_expr,
      &self.ast_builder,
    );
  }

  /// `__rolldown_runtime__.initModule("<stable id>")`
  pub fn make_init_module_call(&self, module: &Module) -> ast::Expression<'ast> {
    Expression::new_call_with_arg(
      Expression::new_member_access_expr("__rolldown_runtime__", "initModule", &self.ast_builder),
      ast::Expression::new_string_literal(
        SPAN,
        Str::from_str_in(module.stable_id(), &self.ast_builder),
        None,
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    )
  }
}

pub struct NamedExport {
  pub local_binding: SymbolId,
}
