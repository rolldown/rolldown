use oxc::{
  allocator::Allocator,
  ast::ast,
  ast_visit::{VisitMut, walk_mut},
};
use rolldown_common::IndexModules;
use rolldown_ecmascript_utils::AstSnippet;

#[expect(unused)]
pub struct HmrAstFinalizer<'me, 'ast> {
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub modules: &'me IndexModules,
}

impl<'ast> VisitMut<'ast> for HmrAstFinalizer<'_, 'ast> {
  fn visit_statement(&mut self, node: &mut ast::Statement<'ast>) {
    if let Some(_module_decl) = node.as_module_declaration_mut() {
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
}
