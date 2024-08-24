use oxc::ast::{ast::*, AstBuilder};
use oxc::span::SPAN;


// pub fn construct_snippet_from_pattern(patter: ImportPattern) -> Statement {}

pub fn construct_snippet_from_import_expr<'a>(
  ast_builder: &AstBuilder<'a>,
  source: Atom<'a>,
  decls: Vec<Atom<'a>>,
  decl_kind: VariableDeclarationKind,
) -> VariableDeclarator<'a> {
  ast_builder.variable_declarator(
    SPAN,
    decl_kind,
    // `const {a, b}`
    //         ^  ^
    ast_builder.binding_pattern(
      ast_builder.binding_pattern_kind_object_pattern(
        SPAN,
        ast_builder.vec_from_iter(decls.iter().map(|name| {
          ast_builder.binding_property(
            SPAN,
            ast_builder.property_key_identifier_name(SPAN, name),
            ast_builder.binding_pattern(
              ast_builder.binding_pattern_kind_binding_identifier(SPAN, name),
              None::<TSTypeAnnotation>,
              false,
            ),
            true,
            false,
          )
        })),
        None::<BindingRestElement>,
      ),
      None::<TSTypeAnnotation>,
      false,
    ),
    Some(ast_builder.expression_await(
      SPAN,
      ast_builder.expression_call(
        SPAN,
        ast_builder.expression_identifier_reference(SPAN, "__vitePreload"),
        None::<TSTypeParameterInstantiation>,
        {
          let mut items = ast_builder.vec();
          items.push(ast_builder.argument_expression(ast_builder.expression_arrow_function(
            SPAN,
            false,
            true,
            None::<TSTypeParameterDeclaration>,
            ast_builder.formal_parameters(
              SPAN,
              FormalParameterKind::ArrowFormalParameters,
              ast_builder.vec(),
              None::<BindingRestElement>,
            ),
            None::<TSTypeAnnotation>,
            ast_builder.function_body(SPAN, ast_builder.vec(), {
              let mut items = ast_builder.vec();
              items.push(ast_builder.statement_declaration(ast_builder.declaration_variable(
                SPAN,
                decl_kind,
                ast_builder.vec1(ast_builder.variable_declarator(
                  SPAN,
                  decl_kind,
                  ast_builder.binding_pattern(
                    ast_builder.binding_pattern_kind_object_pattern(
                      SPAN,
                      ast_builder.vec_from_iter(decls.iter().map(|name| {
                        ast_builder.binding_property(
                          SPAN,
                          ast_builder.property_key_identifier_name(SPAN, name),
                          ast_builder.binding_pattern(
                            ast_builder.binding_pattern_kind_binding_identifier(SPAN, name),
                            None::<TSTypeAnnotation>,
                            false,
                          ),
                          true,
                          false,
                        )
                      })),
                      None::<BindingRestElement>,
                    ),
                    None::<TSTypeAnnotation>,
                    false,
                  ),
                  Some(ast_builder.expression_await(
                    SPAN,
                    ast_builder.expression_import(
                      SPAN,
                      ast_builder.expression_string_literal(SPAN, source),
                      ast_builder.vec(),
                    ),
                  )),
                  false,
                )),
                false,
              )));
              items.push(ast_builder.statement_return(
                SPAN,
                Some(ast_builder.expression_object(
                  SPAN,
                  ast_builder.vec_from_iter(decls.iter().map(|name| {
                    ast_builder.object_property_kind_object_property(
                      SPAN,
                      PropertyKind::Init,
                      ast_builder.property_key_identifier_name(SPAN, name),
                      ast_builder.expression_identifier_reference(SPAN, name),
                      None,
                      false,
                      true,
                      false,
                    )
                  })),
                  None,
                )),
              ));
              items
            }),
          )));
          items.push(ast_builder.argument_expression(ast_builder.expression_conditional(
            SPAN,
            ast_builder.expression_boolean_literal(SPAN, true),
            ast_builder.expression_identifier_reference(SPAN, "__VITE_PRELOAD__"),
            ast_builder.expression_unary(
              SPAN,
              UnaryOperator::Void,
              ast_builder.expression_numeric_literal(SPAN, 0f64, "0", NumberBase::Decimal),
            ),
          )));
          items
        },
        false,
      ),
    )),
    false,
  )
}
