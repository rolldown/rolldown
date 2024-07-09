use itertools::Itertools;
use oxc::allocator::Allocator;
use oxc::ast::ast::{BindingPatternKind, Statement, VariableDeclaration};
use oxc::ast::AstBuilder;
use oxc::span::SPAN;
use rolldown_ecmascript::{EcmaAst, StatementExt, TakeIn, WithMutFields};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub fn tweak_ast_for_scanning(ast: &mut EcmaAst) {
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

    let ast_builder = AstBuilder::new(allocator);
    for stmt in original_body {
      if stmt.is_module_declaration_with_source() {
        program.body.push(stmt);
      } else {
        non_hoisted_statements.extend(split_top_level_variable_decl(stmt, allocator, ast_builder));
      }
    }

    program.body.extend(non_hoisted_statements);
  });
  ast.contains_use_strict = contains_use_strict;
}

fn split_top_level_variable_decl<'a>(
  stmt: Statement<'a>,
  allocator: &'a Allocator,
  ast_builder: AstBuilder<'a>,
) -> Vec<Statement<'a>> {
  match stmt {
    Statement::VariableDeclaration(mut decl) => {
      if decl
        .declarations
        .iter()
        .all(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
      {
        decl
          .declarations
          .take_in(allocator)
          .into_iter()
          .map(|declarator| {
            Statement::VariableDeclaration(ast_builder.variable_declaration(
              SPAN,
              decl.kind,
              ast_builder.new_vec_from_iter([declarator]),
              decl.declare,
            ))
          })
          .collect_vec()
      } else {
        vec![Statement::VariableDeclaration(decl)]
      }
    }
    _ => vec![stmt],
  }
}
