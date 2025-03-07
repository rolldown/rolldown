use oxc::{
  allocator::Allocator,
  ast::ast,
  ast_visit::{VisitMut, walk_mut},
  semantic::SymbolId,
};
use rolldown_common::{IndexModules, Module, NormalModule};
use rolldown_ecmascript_utils::{AstSnippet, BindingIdentifierExt, ExpressionExt, quote_stmt};
use rustc_hash::FxHashMap;

#[expect(unused)]
pub struct HmrAstFinalizer<'me, 'ast> {
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub symbols: &'me oxc::semantic::SymbolTable,
  pub scopes: &'me oxc::semantic::ScopeTree,
  pub modules: &'me IndexModules,
  pub module: &'me NormalModule,
  pub import_binding: FxHashMap<SymbolId, String>,
}

impl<'ast> VisitMut<'ast> for HmrAstFinalizer<'_, 'ast> {
  #[expect(clippy::collapsible_match)]
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
          match &self.modules[self.module.import_records[rec_id].resolved_module] {
            Module::Normal(importee) => {
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
                  _ => {}
                });
              });
              *node = stmt;
            }
            Module::External(_importee) => {
              todo!("handle external module");
            }
          }
        }
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
      let reference = self.symbols.get_reference(ident.reference_id());
      if let Some(symbol_id) = reference.symbol_id() {
        if let Some(binding_name) = self.import_binding.get(&symbol_id) {
          *it = self.snippet.id_ref_expr(binding_name.as_str(), ident.span);
          return;
        }
      }
    }

    walk_mut::walk_expression(self, it);
  }
}
