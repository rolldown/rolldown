use glob::glob;
use oxc::{
  allocator::Vec,
  ast::{
    ast::{
      Argument, ArrayExpressionElement, BindingRestElement, Expression, FormalParameterKind,
      ImportOrExportKind, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
      TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    },
    visit::walk_mut,
    AstBuilder, VisitMut,
  },
  span::{Span, SPAN},
};
use rolldown_plugin::{HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext};
use rustc_hash::FxHashMap;
use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};
use sugar_path::SugarPath;

#[derive(Debug)]
pub struct ImportGlobPlugin {
  pub config: ImportGlobPluginConfig,
}

#[derive(Debug, Default)]
/// vite also support `source_map` config, but we can't support it now.
/// Since the source map now follow the codegen option.
pub struct ImportGlobPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: bool,
}

impl Plugin for ImportGlobPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:import-glob-plugin")
  }

  fn transform_ast(
    &self,
    _ctx: &PluginContext,
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
        source_len: fields.source.len(),
        restore_query_extension: self.config.restore_query_extension,
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
  query: Option<String>,
}

pub struct GlobImportVisit<'ast, 'a> {
  cwd: &'a PathBuf,
  ast_builder: AstBuilder<'ast>,
  import_decls: Vec<'ast, Statement<'ast>>,
  current: usize,
  source_len: usize,
  restore_query_extension: bool,
}

impl<'ast, 'a> VisitMut<'ast> for GlobImportVisit<'ast, 'a> {
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

                  let mut opts = ImportGlobOptions::default();
                  match call_expr.arguments.as_slice() {
                    [first] => self.eval_glob_expr(first, &mut files),
                    // import.meta.glob('./dir/*.js', { import: 'setup' })
                    [first, second] => {
                      self.eval_glob_expr(first, &mut files);
                      extract_import_glob_options(second, &mut opts);
                    }
                    [first, second, _rest @ ..] => {
                      self.eval_glob_expr(first, &mut files);
                      extract_import_glob_options(second, &mut opts);
                    }
                    [] => {}
                  }

                  // generate:
                  //
                  // {
                  //   './dir/ind.js': __glob__0_0_,
                  //   './dir/foo.js': () => import('./dir/foo.js'),
                  //   './dir/bar.js': () => import('./dir/bar.js').then((m) => m.setup),
                  // }

                  *expr = self.generate_glob_object_expression(&files, &opts, call_expr.span);
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

    walk_mut::walk_expression(self, expr);
  }
}

fn extract_import_glob_options(arg: &Argument, opts: &mut ImportGlobOptions) {
  let Argument::ObjectExpression(obj) = arg else {
    return;
  };

  for prop in &obj.properties {
    let ObjectPropertyKind::ObjectProperty(p) = prop else {
      continue;
    };

    let PropertyKind::Init = p.kind else {
      continue;
    };

    let key = match &p.key {
      PropertyKey::StringLiteral(str) => str.value.as_str(),
      PropertyKey::StaticIdentifier(id) => id.name.as_str(),
      _ => continue,
    };

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
      "query" => match &p.value {
        Expression::StringLiteral(str) => {
          opts.query = Some(str.value.to_string());
        }
        Expression::ObjectExpression(expr) => {
          let map = expr
            .properties
            .iter()
            .filter_map(|prop| {
              let ObjectPropertyKind::ObjectProperty(p) = prop else { return None };
              let key = match &p.key {
                PropertyKey::StringLiteral(key) => key.value.to_string(),
                PropertyKey::StaticIdentifier(ident) => ident.name.to_string(),
                _ => return None,
              };
              let value = match &p.value {
                Expression::StringLiteral(v) => v.value.to_string(),
                Expression::BooleanLiteral(v) => v.value.to_string(),
                Expression::NumericLiteral(v) => v.value.to_string(),
                Expression::NullLiteral(_) => "null".to_string(),
                _ => return None,
              };
              Some((key, value.to_string()))
            })
            .collect::<FxHashMap<String, String>>();
          if !map.is_empty() {
            let mut query_string = String::from("?");

            for (i, (k, v)) in map.iter().enumerate() {
              if i != 0 {
                query_string.push('&');
              }
              query_string.push_str(&format!("{k}={v}"));
            }
            opts.query = Some(query_string);
          }
        }
        _ => {}
      },
      _ => {}
    }
  }
}

impl<'ast, 'a> GlobImportVisit<'ast, 'a> {
  fn eval_glob_expr(&mut self, arg: &Argument, files: &mut std::vec::Vec<String>) {
    let mut glob_exprs = vec![];
    match arg {
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
          // TODO handle error
          for file in glob(path).unwrap() {
            let file = file.unwrap().as_path().relative(self.cwd).to_slash_lossy().to_string();
            files.push(format!("./{file}"));
          }
        }
      }
    }
  }

  #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
  fn generate_glob_object_expression(
    &mut self,
    files: &[String],
    opts: &ImportGlobOptions,
    call_expr_span: Span,
  ) -> Expression<'ast> {
    let properties = files.iter().enumerate().map(|(index, file)| {
      let formatted_file = if let Some(query) = &opts.query {
        let normalized_query = if query == "?raw" {
          query
        } else {
          let file_extension =
            Path::new(&file).extension().unwrap_or_default().to_str().unwrap_or_default();
          if !file_extension.is_empty() && self.restore_query_extension {
            &format!("{query}&lang.{file_extension}")
          } else {
            query
          }
        };
        Cow::Owned(format!("{file}{normalized_query}"))
      } else {
        Cow::Borrowed(file)
      };
      let value = if opts.eager.unwrap_or_default() {
        // import * as __glob__0 from './dir/foo.js'
        // const modules = {
        //   './dir/foo.js': __glob__0,
        // }
        let name = format!(
          "__glob__{}_{}_",
          itoa::Buffer::new().format(self.current),
          itoa::Buffer::new().format(index)
        );

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
          Some(import) => self.ast_builder.import_declaration_specifier_import_specifier(
            SPAN,
            self.ast_builder.module_export_name_identifier_reference(SPAN, import),
            self.ast_builder.binding_identifier(SPAN, &name),
            ImportOrExportKind::Value,
          ),
        };

        self.import_decls.push(self.ast_builder.statement_module_declaration(
          self.ast_builder.module_declaration_import_declaration(
            SPAN,
            Some(self.ast_builder.vec1(module_specifier)),
            self.ast_builder.string_literal(Span::default(), formatted_file.as_str()),
            None,
            ImportOrExportKind::Value,
          ),
        ));

        self.ast_builder.expression_identifier_reference(SPAN, &name)
      } else {
        // import('./dir/bar.js')
        let mut import_expression = self.ast_builder.expression_import(
          // Crate a different span for each import expression
          Span::new((self.source_len + self.current) as u32, index as u32),
          self.ast_builder.expression_string_literal(Span::default(), formatted_file.as_str()),
          self.ast_builder.vec(),
        );
        // import('./dir/foo.js').then((m) => m.setup)
        if let Some(import) = &opts.import {
          if import != "*" {
            import_expression = self.ast_builder.expression_call(
              SPAN,
              self.ast_builder.expression_member(self.ast_builder.member_expression_static(
                SPAN,
                import_expression,
                self.ast_builder.identifier_name(SPAN, "then"),
                false,
              )),
              None::<TSTypeParameterInstantiation>,
              self.ast_builder.vec1(
                self
                  .ast_builder
                  .expression_arrow_function(
                    SPAN,
                    true,
                    false,
                    None::<TSTypeParameterDeclaration>,
                    self.ast_builder.formal_parameters(
                      SPAN,
                      FormalParameterKind::ArrowFormalParameters,
                      self.ast_builder.vec1(self.ast_builder.formal_parameter(
                        SPAN,
                        self.ast_builder.vec(),
                        self.ast_builder.binding_pattern(
                          self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "m"),
                          None::<TSTypeAnnotation>,
                          false,
                        ),
                        None,
                        false,
                        false,
                      )),
                      None::<BindingRestElement>,
                    ),
                    None::<TSTypeAnnotation>,
                    self.ast_builder.function_body(
                      SPAN,
                      self.ast_builder.vec(),
                      self.ast_builder.vec1(self.ast_builder.statement_expression(
                        SPAN,
                        self.ast_builder.expression_member(
                          self.ast_builder.member_expression_static(
                            SPAN,
                            self.ast_builder.expression_identifier_reference(SPAN, "m"),
                            self.ast_builder.identifier_name(SPAN, import),
                            false,
                          ),
                        ),
                      )),
                    ),
                  )
                  .into(),
              ),
              false,
            );
          }
        }

        // () => import('./dir/bar.js') or () => import('./dir/foo.js').then((m) => m.setup)
        self.ast_builder.expression_arrow_function(
          SPAN,
          true,
          false,
          None::<TSTypeParameterDeclaration>,
          self.ast_builder.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            self.ast_builder.vec(),
            None::<BindingRestElement>,
          ),
          None::<TSTypeAnnotation>,
          self.ast_builder.function_body(
            SPAN,
            self.ast_builder.vec(),
            self.ast_builder.vec1(self.ast_builder.statement_expression(SPAN, import_expression)),
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

    let properties = self.ast_builder.vec_from_iter(properties);
    self.ast_builder.expression_object(call_expr_span, properties, None)
  }
}
