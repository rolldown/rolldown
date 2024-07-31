use glob::glob;
use oxc::{
  allocator::Vec,
  ast::{
    ast::{
      Argument, ArrayExpressionElement, BindingRestElement, Expression, FormalParameterKind,
      ImportOrExportKind, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
      TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    },
    AstBuilder, VisitMut,
  },
  span::{Span, SPAN},
};
use rolldown_plugin::{HookTransformAstArgs, HookTransformAstReturn, Plugin, SharedPluginContext};
use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};
use sugar_path::SugarPath;

#[derive(Debug)]
pub struct GlobImportPlugin {
  pub config: GlobImportPluginConfig,
}

#[derive(Debug, Default)]
/// vite also support `source_map` config, but we can't support it now.
/// Since the source map now follow the codegen option.
pub struct GlobImportPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: bool,
}

impl Plugin for GlobImportPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("glob_import_plugin")
  }

  fn transform_ast(
    &self,
    _ctx: &SharedPluginContext,
    mut args: HookTransformAstArgs,
  ) -> HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let cwd = self.config.root.as_ref().map(PathBuf::from);
      let mut visitor = GlobImportVisit {
        cwd: cwd.as_ref().unwrap_or(args.cwd),
        import_decls: ast_builder.vec(),
        ast_builder,
        current: 0,
      };
      visitor.visit_program(fields.program);
      if !visitor.import_decls.is_empty() {
        fields.program.body.extend(visitor.import_decls);
      }
    });
    Ok(args.ast)
  }
}

#[derive(Debug, Default)]
pub struct ImportGlobOptions {
  import: Option<String>,
  eager: Option<bool>,
}

pub struct GlobImportVisit<'ast, 'a> {
  cwd: &'a PathBuf,
  ast_builder: AstBuilder<'ast>,
  import_decls: Vec<'ast, Statement<'ast>>,
  current: usize,
}

impl<'ast, 'a> VisitMut<'ast> for GlobImportVisit<'ast, 'a> {
  #[allow(clippy::too_many_lines)]
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if let Expression::CallExpression(call_expr) = expr {
      match &call_expr.callee {
        Expression::StaticMemberExpression(e) => {
          if e.property.name == "glob" {
            match &e.object {
              Expression::MetaProperty(p) => {
                if p.meta.name == "import" && p.property.name == "meta" {
                  let mut files = vec![];
                  // import.meta.glob('./dir/*.js')
                  // import.meta.glob(['./dir/*.js', './dir2/*.js'])
                  if let Some(expr) = call_expr.arguments.first() {
                    let mut glob_exprs = vec![];
                    match expr {
                      Argument::StringLiteral(str) => {
                        glob_exprs.push(str.value.as_str());
                      }
                      Argument::ArrayExpression(array_expr) => {
                        for expr in &array_expr.elements {
                          if let ArrayExpressionElement::StringLiteral(str) = expr {
                            glob_exprs.push(str.value.as_str());
                          }
                        }
                      }
                      _ => {}
                    }

                    for glob_expr in glob_exprs {
                      let path = Path::new(self.cwd).join(Path::new(glob_expr));
                      if path.is_absolute() {
                        if let Some(path) = path.to_str() {
                          for file in glob(path).unwrap() {
                            let file = file
                              .unwrap()
                              .as_path()
                              .relative(self.cwd)
                              .to_slash_lossy()
                              .to_string();
                            files.push(format!("./{file}"));
                          }
                        }
                      }
                    }
                  }

                  // import.meta.glob('./dir/*.js', { import: 'setup' })
                  let mut opts = ImportGlobOptions::default();
                  if let Some(Argument::ObjectExpression(obj)) = call_expr.arguments.get(1) {
                    for prop in &obj.properties {
                      if let ObjectPropertyKind::ObjectProperty(p) = prop {
                        if let PropertyKind::Init = p.kind {
                          if let Some(key) = match &p.key {
                            PropertyKey::StringLiteral(str) => Some(str.value.as_str()),
                            PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
                            _ => None,
                          } {
                            match key {
                              "import" => {
                                if let Expression::StringLiteral(str) = &p.value {
                                  opts.import = Some(str.value.as_str().to_string());
                                }
                              }
                              "eager" => {
                                if let Expression::BooleanLiteral(bool) = &p.value {
                                  opts.eager = Some(bool.value);
                                }
                              }
                              _ => {}
                            }
                          }
                        }
                      }
                    }
                  }

                  // {
                  //   './dir/ind.js': __glob__0_0_,
                  //   './dir/foo.js': () => import('./dir/foo.js'),
                  //   './dir/bar.js': () => import('./dir/bar.js').then((m) => m.setup),
                  // }
                  let properties = files.iter().enumerate().map(|(index, file)| {
                    let value = if opts.eager.unwrap_or_default() {
                      // import * as __glob__0 from './dir/foo.js'
                      // const modules = {
                      //   './dir/foo.js': __glob__0,
                      // }
                      let name = format!("__glob__{}_{index}_", self.current);

                      let module_specifier = match opts.import.as_deref() {
                        Some("default") => {
                          self.ast_builder.import_declaration_specifier_import_default_specifier(
                            SPAN,
                            self.ast_builder.binding_identifier(SPAN, &name),
                          )
                        }
                        Some("*") | None => {
                          self.ast_builder.import_declaration_specifier_import_namespace_specifier(
                            SPAN,
                            self.ast_builder.binding_identifier(SPAN, &name),
                          )
                        }
                        Some(import) => {
                          self.ast_builder.import_declaration_specifier_import_specifier(
                            SPAN,
                            self.ast_builder.module_export_name_identifier_reference(SPAN, import),
                            self.ast_builder.binding_identifier(SPAN, &name),
                            ImportOrExportKind::Value,
                          )
                        }
                      };

                      self.import_decls.push(self.ast_builder.statement_module_declaration(
                        self.ast_builder.module_declaration_import_declaration(
                          SPAN,
                          Some(self.ast_builder.vec1(module_specifier)),
                          self.ast_builder.string_literal(Span::default(), file),
                          None,
                          ImportOrExportKind::Value,
                        ),
                      ));

                      self.ast_builder.expression_identifier_reference(SPAN, &name)
                    } else {
                      // import('./dir/bar.js')
                      let mut import_expression = self.ast_builder.expression_import(
                        SPAN,
                        self.ast_builder.expression_string_literal(Span::default(), file),
                        self.ast_builder.vec(),
                      );
                      // import('./dir/foo.js').then((m) => m.setup)
                      if let Some(import) = &opts.import {
                        if import != "*" {
                          import_expression = self.ast_builder.expression_call(
                            SPAN,
                            self.ast_builder.vec1(
                              self
                                .ast_builder
                                .expression_arrow_function(
                                  SPAN,
                                  true,
                                  false,
                                  Option::<TSTypeParameterDeclaration>::None,
                                  self.ast_builder.formal_parameters(
                                    SPAN,
                                    FormalParameterKind::ArrowFormalParameters,
                                    self.ast_builder.vec1(
                                      self.ast_builder.formal_parameter(
                                        SPAN,
                                        self.ast_builder.vec(),
                                        self.ast_builder.binding_pattern(
                                          self
                                            .ast_builder
                                            .binding_pattern_kind_binding_identifier(SPAN, "m"),
                                          Option::<TSTypeAnnotation>::None,
                                          false,
                                        ),
                                        None,
                                        false,
                                        false,
                                      ),
                                    ),
                                    Option::<BindingRestElement>::None,
                                  ),
                                  Option::<TSTypeAnnotation>::None,
                                  self.ast_builder.function_body(
                                    SPAN,
                                    self.ast_builder.vec(),
                                    self.ast_builder.vec1(
                                      self.ast_builder.statement_expression(
                                        SPAN,
                                        self.ast_builder.expression_member(
                                          self.ast_builder.member_expression_static(
                                            SPAN,
                                            self
                                              .ast_builder
                                              .expression_identifier_reference(SPAN, "m"),
                                            self.ast_builder.identifier_name(SPAN, import),
                                            false,
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                )
                                .into(),
                            ),
                            self.ast_builder.expression_member(
                              self.ast_builder.member_expression_static(
                                SPAN,
                                import_expression,
                                self.ast_builder.identifier_name(SPAN, "then"),
                                false,
                              ),
                            ),
                            Option::<TSTypeParameterInstantiation>::None,
                            false,
                          );
                        }
                      }

                      // () => import('./dir/bar.js') or () => import('./dir/foo.js').then((m) => m.setup)
                      self.ast_builder.expression_arrow_function(
                        SPAN,
                        true,
                        false,
                        Option::<TSTypeParameterDeclaration>::None,
                        self.ast_builder.formal_parameters(
                          SPAN,
                          FormalParameterKind::ArrowFormalParameters,
                          self.ast_builder.vec(),
                          Option::<BindingRestElement>::None,
                        ),
                        Option::<TSTypeAnnotation>::None,
                        self.ast_builder.function_body(
                          SPAN,
                          self.ast_builder.vec(),
                          self
                            .ast_builder
                            .vec1(self.ast_builder.statement_expression(SPAN, import_expression)),
                        ),
                      )
                    };

                    self.ast_builder.object_property_kind_object_property(
                      SPAN,
                      PropertyKind::Init,
                      self.ast_builder.property_key_expression(
                        self.ast_builder.expression_string_literal(Span::default(), file),
                      ),
                      value,
                      None,
                      false,
                      false,
                      false,
                    )
                  });

                  *expr = self.ast_builder.expression_object(
                    call_expr.span,
                    self.ast_builder.vec_from_iter(properties),
                    None,
                  );
                  self.current += 1;
                }
              }
              _ => {}
            }
          }
        }
        _ => {}
      }
    }
  }
}
