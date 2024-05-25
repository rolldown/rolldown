use rolldown_oxc_utils::{OxcAst, StatementExt, TakeIn, WithFieldsMut};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub fn tweak_ast_for_scanning(ast: &mut OxcAst) {
  ast.with_mut(|WithFieldsMut { program, allocator, .. }| {
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
}
