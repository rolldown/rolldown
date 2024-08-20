use itertools::Itertools;
use oxc::allocator::Allocator;
use oxc::ast::ast::{BindingPatternKind, Declaration, Statement};
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
        non_hoisted_statements.extend(split_top_level_variable_declaration(
          stmt,
          allocator,
          ast_builder,
        ));
      }
    }

    program.body.extend(non_hoisted_statements);
  });
  ast.contains_use_strict = contains_use_strict;
}

fn split_top_level_variable_declaration<'a>(
  stmt: Statement<'a>,
  allocator: &'a Allocator,
  ast_builder: AstBuilder<'a>,
) -> Vec<Statement<'a>> {
  match stmt {
    Statement::ExportNamedDeclaration(mut named_decl) => {
      let named_decl_export_kind = named_decl.export_kind;
      let named_decl_span = named_decl.span;
      let Some(Declaration::VariableDeclaration(ref mut var_decl)) = named_decl.declaration else {
        return vec![Statement::ExportNamedDeclaration(named_decl)];
      };

      if var_decl
        .declarations
        .iter()
        // TODO: support nested destructuring tree shake, `export const {a, b} = obj; export const
        // [a, b] = arr;`
        .any(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
      {
        var_decl
          .declarations
          .take_in(allocator)
          .into_iter()
          .enumerate()
          .map(|(i, declarator)| {
            let is_first = i == 0;
            let new_decl = ast_builder.alloc_variable_declaration(
              SPAN,
              var_decl.kind,
              ast_builder.vec_from_iter([declarator]),
              var_decl.declare,
            );
            Statement::ExportNamedDeclaration(ast_builder.alloc_export_named_declaration(
              if is_first { named_decl_span } else { SPAN },
              Some(Declaration::VariableDeclaration(new_decl)),
              ast_builder.vec(),
              // Since it is `export a = 1, b = 2;`, source should be `None`
              None,
              named_decl_export_kind,
              None,
            ))
          })
          .collect_vec()
      } else {
        vec![Statement::ExportNamedDeclaration(named_decl)]
      }
    }
    _ => vec![stmt],
  }
}
