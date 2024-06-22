use rolldown_oxc_utils::{OxcAst, StatementExt, TakeIn, WithMutFields};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub fn tweak_ast_for_scanning(ast: &mut OxcAst) {
  let mut contains_use_strict = false;
  ast.program.with_mut(|WithMutFields { program, allocator, .. }| {
    // Remove all `"use strict"` directives.
    program.directives.retain(|directive| {
      let is_use_strict = directive.is_use_strict();
      if is_use_strict {
        contains_use_strict = true;
        false
      } else {
        true
      }
    });

    let original_body = program.body.take_in(allocator);
    program.body.reserve_exact(original_body.len());
    let mut non_hoisted_statements = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for stmt in original_body {
      if stmt.is_module_declaration_with_source() {
        program.body.push(stmt);
      } else {
        non_hoisted_statements.push(stmt);
      }
    }

    program.body.extend(non_hoisted_statements);
  });
  ast.contains_use_strict = contains_use_strict;
}
