use oxc::{
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, Atom, Expression, FormalParameterKind, PropertyKind, Statement,
      VariableDeclarationKind, VariableDeclarator,
    },
  },
  span::SPAN,
};

const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";

pub fn construct_snippet_from_await_decl<'a>(
  ast_builder: AstBuilder<'a>,
  source: Atom<'a>,
  decls: &[Atom<'a>],
  decl_kind: VariableDeclarationKind,
  append_import_meta_url: bool,
) -> VariableDeclarator<'a> {
  ast_builder.variable_declarator(
    SPAN,
    decl_kind,
    // `const {a, b}`
    //         ^  ^
    ast_builder.binding_pattern(
      ast_builder.binding_pattern_kind_object_pattern(
        SPAN,
        ast_builder.vec_from_iter(decls.iter().map(|&name| {
          ast_builder.binding_property(
            SPAN,
            ast_builder.property_key_static_identifier(SPAN, name),
            ast_builder.binding_pattern(
              ast_builder.binding_pattern_kind_binding_identifier(SPAN, name),
              NONE,
              false,
            ),
            true,
            false,
          )
        })),
        NONE,
      ),
      NONE,
      false,
    ),
    Some(ast_builder.expression_await(
      SPAN,
      construct_vite_preload_call(ast_builder, decl_kind, decls, source, append_import_meta_url),
    )),
    false,
  )
}

#[allow(clippy::too_many_lines)]
/// generate `__vitePreload(async () => { const {foo} = await import('foo');return { foo }},...)`
fn construct_vite_preload_call<'a>(
  ast_builder: AstBuilder<'a>,
  decl_kind: VariableDeclarationKind,
  decls: &[Atom<'a>],
  source: Atom<'a>,
  append_import_meta_url: bool,
) -> Expression<'a> {
  ast_builder.expression_call(
    SPAN,
    ast_builder.expression_identifier(SPAN, "__vitePreload"),
    NONE,
    {
      let mut items = ast_builder.vec();
      items.push(Argument::from(ast_builder.expression_arrow_function(
        SPAN,
        false,
        true,
        NONE,
        ast_builder.formal_parameters(
          SPAN,
          FormalParameterKind::ArrowFormalParameters,
          ast_builder.vec(),
          NONE,
        ),
        NONE,
        ast_builder.function_body(SPAN, ast_builder.vec(), {
          let mut items = ast_builder.vec();
          items.push(Statement::from(ast_builder.declaration_variable(
            SPAN,
            decl_kind,
            ast_builder.vec1(ast_builder.variable_declarator(
              SPAN,
              decl_kind,
              ast_builder.binding_pattern(
                ast_builder.binding_pattern_kind_object_pattern(
                  SPAN,
                  ast_builder.vec_from_iter(decls.iter().map(|&name| {
                    ast_builder.binding_property(
                      SPAN,
                      ast_builder.property_key_static_identifier(SPAN, name),
                      ast_builder.binding_pattern(
                        ast_builder.binding_pattern_kind_binding_identifier(SPAN, name),
                        NONE,
                        false,
                      ),
                      true,
                      false,
                    )
                  })),
                  NONE,
                ),
                NONE,
                false,
              ),
              Some(ast_builder.expression_await(
                SPAN,
                ast_builder.expression_import(
                  SPAN,
                  ast_builder.expression_string_literal(SPAN, source, None),
                  None,
                  None,
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
              ast_builder.vec_from_iter(decls.iter().map(|&name| {
                ast_builder.object_property_kind_object_property(
                  SPAN,
                  PropertyKind::Init,
                  ast_builder.property_key_static_identifier(SPAN, name),
                  ast_builder.expression_identifier(SPAN, name),
                  false,
                  true,
                  false,
                )
              })),
            )),
          ));
          items
        }),
      )));
      items.push(Argument::from(ast_builder.expression_conditional(
        SPAN,
        ast_builder.expression_identifier(SPAN, IS_MODERN_FLAG),
        ast_builder.expression_identifier(SPAN, "__VITE_PRELOAD__"),
        ast_builder.void_0(SPAN),
      )));
      if append_import_meta_url {
        items.push(Argument::from(Expression::from(ast_builder.member_expression_static(
          SPAN,
          ast_builder.expression_meta_property(
            SPAN,
            ast_builder.identifier_name(SPAN, "import"),
            ast_builder.identifier_name(SPAN, "meta"),
          ),
          ast_builder.identifier_name(SPAN, "url"),
          false,
        ))));
      }
      items
    },
    false,
  )
}

/// 1.transform `import('foo').then(({foo})=>{})`
///   to `__vitePreload(async () => { const {foo} = await import('foo');return { foo }},...).then(({foo})=>{})`
/// 2.transform `(await import('foo')).foo`
///   to `__vitePreload(async () => { const {foo} = (await import('foo')); return { foo }},...)).foo`
pub fn construct_snippet_for_expression<'a>(
  ast_builder: AstBuilder<'a>,
  source: Atom<'a>,
  decls: &[Atom<'a>],
  append_import_meta_url: bool,
) -> Expression<'a> {
  construct_vite_preload_call(
    ast_builder,
    VariableDeclarationKind::Const,
    decls,
    source,
    append_import_meta_url,
  )
}
