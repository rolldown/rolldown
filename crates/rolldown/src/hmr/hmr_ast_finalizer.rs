use oxc::{
  allocator::{Allocator, Box as ArenaBox, IntoIn, TakeIn},
  ast::{
    AstBuilder, NONE,
    ast::{self, ExportDefaultDeclarationKind, Expression, ObjectPropertyKind, Statement},
  },
  semantic::{IsGlobalReference, Scoping, SymbolId},
  span::{Atom, SPAN, Span},
};

use rolldown_common::{
  ExternalModule, ImportRecordIdx, IndexModules, Module, ModuleIdx, NormalModule,
};
use rolldown_ecmascript::CJS_REQUIRE_REF_ATOM;
use rolldown_ecmascript_utils::{AstSnippet, BindingIdentifierExt, ExpressionExt};
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::hmr::utils::{HmrAstBuilder, MODULE_EXPORTS_NAME_FOR_ESM};

pub struct HmrAstFinalizer<'me, 'ast> {
  // Outside input
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub builder: &'me AstBuilder<'ast>,
  pub modules: &'me IndexModules,
  pub module: &'me NormalModule,
  pub affected_module_idx_to_init_fn_name: &'me FxHashMap<ModuleIdx, String>,
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
  pub named_exports: FxHashMap<Atom<'ast>, NamedExport>,
}

impl<'ast> HmrAstFinalizer<'_, 'ast> {
  #[expect(clippy::too_many_lines)]
  pub fn handle_top_level_stmt(
    &mut self,
    program_body: &mut oxc::allocator::Vec<'ast, ast::Statement<'ast>>,
    mut node: ast::Statement<'ast>,
    scoping: &Scoping,
  ) {
    match node {
      ref mut module_decl @ ast::match_module_declaration!(Statement) => {
        let module_decl = module_decl.to_module_declaration_mut();

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
            let binding_name =
              self.ensure_static_import_info(rec.resolved_module, rec_id).to_string();
            import_decl.specifiers.as_ref().inspect(|specifiers| {
              specifiers.iter().for_each(|spec| match spec {
                ast::ImportDeclarationSpecifier::ImportSpecifier(import_specifier) => {
                  self.import_bindings.insert(
                    import_specifier.local.expect_symbol_id(),
                    format!("{binding_name}.{}", import_specifier.imported.name()),
                  );
                }
                ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(
                  import_default_specifier,
                ) => {
                  self.import_bindings.insert(
                    import_default_specifier.local.expect_symbol_id(),
                    format!("{binding_name}.default"),
                  );
                }
                ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(
                  import_namespace_specifier,
                ) => {
                  self.import_bindings.insert(
                    import_namespace_specifier.local.expect_symbol_id(),
                    binding_name.to_string(),
                  );
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
          ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
            if let Some(_source) = &decl.source {
              // export {} from '...'
              let rec_id = self.module.imports[&decl.span];
              let rec = &self.module.import_records[rec_id];
              let importee = &self.modules[rec.resolved_module];
              self.dependencies.insert(rec.resolved_module);

              let binding_name =
                self.ensure_static_import_info(rec.resolved_module, rec_id).to_string();
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
                program_body.push(stmt);
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
              program_body.push(ast::Statement::from(decl.take_in(self.alloc)));
            } else {
              // export { foo, bar as bar2 }
              decl.specifiers.iter().for_each(|specifier| {
                if let Some(symbol_id) = scoping.get_root_binding(&specifier.local.name()) {
                  self
                    .named_exports
                    .insert(specifier.exported.name(), NamedExport { local_binding: symbol_id });
                } else {
                  // TODO: export undefined variable
                }
              });
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
              program_body.push(ast::Statement::FunctionDeclaration(ArenaBox::new_in(
                function.as_mut().take_in(self.alloc),
                self.alloc,
              )));
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
              program_body.push(ast::Statement::ClassDeclaration(ArenaBox::new_in(
                class.as_mut().take_in(self.alloc),
                self.alloc,
              )));
            }
            expr @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              let expr = expr.to_expression_mut();
              // Transform `export default [expression]` => `var __rolldown_default__ = [expression]`
              program_body
                .push(self.snippet.var_decl_stmt("__rolldown_default__", expr.take_in(self.alloc)));
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
          ast::ModuleDeclaration::ExportAllDeclaration(export_all_decl) => {
            let rec_id = self.module.imports[&export_all_decl.span];
            let rec = &self.module.import_records[rec_id];
            let importee = &self.modules[rec.resolved_module];
            self.dependencies.insert(rec.resolved_module);
            let binding_name =
              self.ensure_static_import_info(rec.resolved_module, rec_id).to_string();
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
          ast::ModuleDeclaration::TSExportAssignment(_)
          | ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => {
            // Typescript code should be pre-processed by rolldown. If it doesn't, it means there's error. Instead of panic, we'll just keep
            // the original code.
            program_body.push(node);
          }
        }
      }
      _ => {
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

    // let module_exports =
    //   match self.module.exports_kind {
    //     rolldown_common::ExportsKind::Esm => {
    //       ret.extend(self.generate_declaration_of_module_namespace_object(
    //         &binding_name_for_namespace_object_ref,
    //       ));

    //       // { exports: namespace }
    //       ast::Argument::ObjectExpression(self.snippet.builder.alloc_object_expression(
    //         SPAN,
    //         self.snippet.builder.vec1(self.snippet.builder.object_property_kind_object_property(
    //           SPAN,
    //           PropertyKind::Init,
    //           self.snippet.builder.property_key_static_identifier(SPAN, "exports"),
    //           self.snippet.id_ref_expr(&binding_name_for_namespace_object_ref, SPAN),
    //           true,
    //           false,
    //           false,
    //         )),
    //       ))
    //     }
    //     rolldown_common::ExportsKind::CommonJs => {
    //       // `module`
    //       ast::Argument::Identifier(self.snippet.builder.alloc_identifier_reference(SPAN, "module"))
    //     }
    //     rolldown_common::ExportsKind::None => ast::Argument::ObjectExpression(
    //       // `{}`
    //       self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec()),
    //     ),
    //   };

    // // __rolldown_runtime__.registerModule(moduleId, module)
    // let arguments = self.snippet.builder.vec_from_array([
    //   ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
    //     SPAN,
    //     self.snippet.builder.atom(&self.module.stable_id),
    //     None,
    //   )),
    //   module_exports,
    // ]);

    // let register_call = self.snippet.builder.alloc_call_expression(
    //   SPAN,
    //   self.snippet.id_ref_expr("__rolldown_runtime__.registerModule", SPAN),
    //   NONE,
    //   arguments,
    //   false,
    // );

    // ret.push(ast::Statement::ExpressionStatement(
    //   self
    //     .snippet
    //     .builder
    //     .alloc_expression_statement(SPAN, ast::Expression::CallExpression(register_call)),
    // ));

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
    let call_expr = self.snippet.call_expr_with_arg_expr(
      self.snippet.literal_prop_access_member_expr_expr("__rolldown_runtime__", "loadExports"),
      self.snippet.string_literal_expr(id, SPAN),
      false,
    );

    let stmt = self.snippet.variable_declarator_require_call_stmt(
      binding_name,
      self.snippet.to_esm_call_with_interop("__rolldown_runtime__.__toESM", call_expr, interop),
      span,
    );
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
    let stmt = self.snippet.import_star_stmt(module_request, binding_name);

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

    let call_expr = self.snippet.call_expr_with_2arg_expr(
      self.snippet.literal_prop_access_member_expr_expr("__rolldown_runtime__", "__reExport"),
      self.snippet.id_ref_expr(self_exports, SPAN),
      self.snippet.id_ref_expr(binding_name, SPAN),
    );

    Some(ast::Statement::ExpressionStatement(
      self.snippet.builder.alloc_expression_statement(span, call_expr),
    ))
  }

  fn generate_declaration_of_module_namespace_object(
    &mut self,
    binding_name_for_namespace_object_ref: &str,
    scoping: &Scoping,
  ) -> Vec<ast::Statement<'ast>> {
    // TODO reexport external module

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = self
      .snippet
      .builder
      .alloc_object_expression(SPAN, self.snippet.builder.vec_with_capacity(self.exports.len()));
    arg_obj_expr.properties.extend(self.exports.drain(..));
    arg_obj_expr.properties.extend(self.named_exports.iter().map(|(exported, named_export)| {
      let expr = if let Some(local_binding) = self.import_bindings.get(&named_export.local_binding)
      {
        self.snippet.id_ref_expr(local_binding, SPAN)
      } else {
        let name = scoping.symbol_name(named_export.local_binding);
        self.snippet.id_ref_expr(name, SPAN)
      };
      self.snippet.object_property_kind_object_property(exported, expr, false)
    }));

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let export_call_expr = self.snippet.builder.expression_call(
      SPAN,
      self.snippet.id_ref_expr("__rolldown_runtime__.__export", SPAN),
      NONE,
      self
        .snippet
        .builder
        .vec_from_array([ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc))]),
      false,
    );

    // construct `var [binding_name_for_namespace_object_ref] = __export({ prop_name: () => returned, ... })`
    let decl_stmt =
      self.snippet.var_decl_stmt(binding_name_for_namespace_object_ref, export_call_expr);
    vec![decl_stmt]
  }

  // Rewrite `import(...)` to sensible form.
  pub fn try_rewrite_dynamic_import(&self, it: &mut ast::Expression<'ast>) {
    let ast::Expression::ImportExpression(import_expr) = it else {
      return;
    };

    let Some(rec_idx) = self.module.imports.get(&import_expr.span) else {
      return;
    };

    let importee_idx = &self.module.import_records[*rec_idx].resolved_module;

    let Module::Normal(importee) = &self.modules[*importee_idx] else {
      // Not a normal module, skip
      return;
    };
    // FIXME: consider about CommonJS interop
    let is_importee_cjs = importee.exports_kind == rolldown_common::ExportsKind::CommonJs;

    // __rolldown_runtime__.loadExports('./foo.js')
    let mut load_exports_call_expr =
      ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
        SPAN,
        self.snippet.id_ref_expr("__rolldown_runtime__.loadExports", SPAN),
        NONE,
        self.snippet.builder.vec1(ast::Argument::StringLiteral(
          self.snippet.builder.alloc_string_literal(
            SPAN,
            self.snippet.builder.atom(&importee.stable_id),
            None,
          ),
        )),
        false,
      ));

    if is_importee_cjs {
      let is_node_cjs = importee.def_format.is_commonjs();

      let mut args = self.snippet.builder.vec1(ast::Argument::from(load_exports_call_expr));
      if is_node_cjs {
        args.push(ast::Argument::from(self.snippet.builder.expression_numeric_literal(
          SPAN,
          1.0,
          None,
          ast::NumberBase::Decimal,
        )));
      }

      // __rolldown_runtime__.__toDynamicImportESM(__rolldown_runtime__.loadExports('./foo.js'), node_mode)
      load_exports_call_expr = self.snippet.builder.expression_call(
        SPAN,
        self.snippet.id_ref_expr("__rolldown_runtime__.__toDynamicImportESM", SPAN),
        NONE,
        args,
        false,
      );
    }

    if let Some(init_fn_name) = self.affected_module_idx_to_init_fn_name.get(importee_idx) {
      // If the importee is in the propagation chain, we need to call the init function to re-execute the module.
      // Turn `import('./foo.js')` into `(init_foo(), Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js')))`

      // init_foo()
      let init_fn_call = self.snippet.builder.alloc_call_expression(
        SPAN,
        self.snippet.id_ref_expr(init_fn_name, SPAN),
        NONE,
        self.snippet.builder.vec(),
        false,
      );

      // Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js'))
      let promise_resolve_then_load_exports =
        self.snippet.promise_resolve_then_call_expr(load_exports_call_expr);

      // (init_foo(), Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js')))
      let ret_expr =
        ast::Expression::SequenceExpression(self.snippet.builder.alloc_sequence_expression(
          SPAN,
          self.snippet.builder.vec_from_array([
            ast::Expression::CallExpression(init_fn_call),
            promise_resolve_then_load_exports,
          ]),
        ));
      *it = ret_expr;
    } else {
      // Turn `import('./foo.js')` into `Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js'))`

      // `Promise.resolve().then(() => __rolldown_runtime__.loadExports('./foo.js'))`
      *it = self.snippet.promise_resolve_then_call_expr(load_exports_call_expr);
    }
  }

  pub fn try_rewrite_require(
    &self,
    it: &mut ast::Expression<'ast>,
    ctx: &oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let scoping = ctx.scoping();

    // Rewrite standalone `require` to `__rolldown_runtime__.loadExports`
    if let Some(id_ref) = it.as_identifier()
      && id_ref.name == CJS_REQUIRE_REF_ATOM
      && id_ref.is_global_reference(scoping)
      && !ctx.parent().is_call_expression()
    {
      *it =
        self.snippet.literal_prop_access_member_expr_expr("__rolldown_runtime__", "loadExports");
    }

    // Rewrite `require(...)` to `(require_xxx(), __rolldown_runtime__.loadExports())` or keep it as is for external module importee.
    let ast::Expression::CallExpression(call_expr) = it else {
      return;
    };

    if !call_expr
      .callee
      .as_identifier()
      .is_some_and(|id| id.name == CJS_REQUIRE_REF_ATOM && id.is_global_reference(scoping))
    {
      return;
    }

    let Some(rec_idx) = self.module.imports.get(&call_expr.span) else {
      return;
    };

    let importee_idx = &self.module.import_records[*rec_idx].resolved_module;

    let Module::Normal(importee) = &self.modules[*importee_idx] else {
      // Not a normal module, skip
      return;
    };

    let is_importee_cjs = importee.exports_kind == rolldown_common::ExportsKind::CommonJs;

    let init_fn_name = &self.affected_module_idx_to_init_fn_name[importee_idx];

    if is_importee_cjs {
      *it = self.snippet.seq2_in_paren_expr(
        self.snippet.call_expr_expr(init_fn_name),
        self.snippet.call_expr_with_arg_expr(
          self.snippet.literal_prop_access_member_expr_expr("__rolldown_runtime__", "loadExports"),
          self.snippet.string_literal_expr(&importee.stable_id, SPAN),
          false,
        ),
      );
    } else {
      // hyf0 TODO: handle esm importee
      *it = self.snippet.seq2_in_paren_expr(
        self.snippet.call_expr_expr(init_fn_name),
        self.snippet.call_expr_with_arg_expr(
          self.snippet.literal_prop_access_member_expr_expr("__rolldown_runtime__", "loadExports"),
          self.snippet.string_literal_expr(&importee.stable_id, SPAN),
          false,
        ),
      );
    }
  }
}

pub struct NamedExport {
  pub local_binding: SymbolId,
}
