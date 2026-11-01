use oxc::ast::ast::{ImportDeclarationSpecifier, ImportPhase, Statement};
use rolldown_common::Module;

use super::LinkStage;

impl LinkStage<'_> {
  pub(super) fn collect_deferred_imports(&mut self) {
    for module in self.module_table.iter().filter_map(Module::as_normal) {
      let Some(ast) = &self.ast_table[module.idx] else {
        continue;
      };
      for statement in &ast.program().body {
        let Statement::ImportDeclaration(import) = statement else {
          continue;
        };
        if import.phase != Some(ImportPhase::Defer) {
          continue;
        }
        let record = module.imports[&import.node_id()];
        if let Some(specifiers) = &import.specifiers {
          for specifier in specifiers {
            if let ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) = specifier {
              self.metas[module.idx]
                .deferred_imports
                .insert(record, (module.idx, namespace.local.symbol_id()).into());
            }
          }
        }
      }
    }
  }
}
