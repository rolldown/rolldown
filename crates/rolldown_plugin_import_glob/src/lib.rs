use std::{
  borrow::Cow,
  fmt::Write as _,
  path::{Path, PathBuf},
};

use glob::{Pattern, glob};
use oxc::{
  allocator::Vec,
  ast::{
    AstBuilder, NONE,
    ast::{
      Argument, ArrayExpressionElement, Expression, FormalParameterKind, ImportOrExportKind,
      NumberBase, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    },
  },
  ast_visit::{VisitMut, walk_mut},
  span::{SPAN, Span},
};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_plugin::{HookTransformAstArgs, HookTransformAstReturn, Plugin, PluginContext};
use rustc_hash::FxHashMap;
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

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let normalized_id = args.id.to_slash_lossy();
      let root = self.config.root.as_ref().map(PathBuf::from);
      let mut visitor = GlobImportVisit {
        root: root.as_ref().unwrap_or(args.cwd),
        import_decls: ast_builder.vec(),
        ast_builder,
        current: 0,
        restore_query_extension: self.config.restore_query_extension,
        id: &normalized_id,
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
  root: &'a PathBuf,
  ast_builder: AstBuilder<'ast>,
  import_decls: Vec<'ast, Statement<'ast>>,
  current: usize,
  restore_query_extension: bool,
  id: &'a str,
}

impl<'ast> VisitMut<'ast> for GlobImportVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if !self.maybe_visit_obj_call(expr).unwrap_or_default() {
      self.maybe_visit_glob_import_call(expr, None);
    }
    walk_mut::walk_expression(self, expr);
  }
}

#[derive(Debug, PartialEq, Eq)]
enum OmitType {
  Keys,
  Values,
}

impl<'ast> GlobImportVisit<'ast, '_> {
  fn maybe_visit_obj_call(&mut self, expr: &mut Expression<'ast>) -> Option<bool> {
    let call_expr = expr.as_call_expression_mut()?;
    let member_expr = call_expr.callee.as_static_member_expr_mut()?;

    let property_name = member_expr.property.name;
    if property_name != "keys" && property_name != "values" {
      return None;
    }
    let ident = member_expr.object.as_identifier()?;
    // TODO: check is_global_identifier_reference
    if ident.name != "Object" {
      return None;
    }
    let [arg] = call_expr.arguments.as_mut_slice() else { return None };
    let arg_expr = arg.as_expression_mut()?;
    self.maybe_visit_glob_import_call(
      arg_expr,
      Some(if property_name == "keys" { OmitType::Values } else { OmitType::Keys }).as_ref(),
    );
    Some(true)
  }

  fn maybe_visit_glob_import_call(
    &mut self,
    expr: &mut Expression<'ast>,
    omit_type: Option<&OmitType>,
  ) {
    let omit_keys = omit_type == Some(&OmitType::Keys);
    let omit_values = omit_type == Some(&OmitType::Values);

    let Expression::CallExpression(call_expr) = expr else { return };
    let Expression::StaticMemberExpression(callee) = &call_expr.callee else { return };
    if callee.property.name != "glob" {
      return;
    }
    let Expression::MetaProperty(p) = &callee.object else { return };
    if p.meta.name != "import" || p.property.name != "meta" {
      return;
    }
    let mut files = vec![];
    // import.meta.glob('./dir/*.js')
    // import.meta.glob(['./dir/*.js', './dir2/*.js'])

    let mut opts = ImportGlobOptions::default();
    match call_expr.arguments.as_slice() {
      [first] => self.eval_glob_expr(first, &mut files),
      // import.meta.glob('./dir/*.js', { import: 'setup' })
      [first, second, ..] => {
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
    *expr =
      self.generate_glob_object_expression(&files, &opts, call_expr.span, omit_keys, omit_values);
    self.current += 1;
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
          opts.query = if str.value.starts_with('?') {
            Some(str.value.to_string())
          } else {
            Some(format!("?{}", str.value))
          }
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
              Some((key, value))
            })
            .collect::<FxHashMap<String, String>>();
          if !map.is_empty() {
            let mut query_string = String::from("?");

            for (i, (k, v)) in map.iter().enumerate() {
              if i != 0 {
                query_string.push('&');
              }
              write!(query_string, "{k}={v}").unwrap();
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

struct FileData {
  key: String,
  import_path: String,
}

impl<'ast> GlobImportVisit<'ast, '_> {
  fn eval_glob_expr(&self, arg: &Argument, files: &mut std::vec::Vec<FileData>) {
    let mut positive_globs = vec![];
    let mut negated_globs = vec![];
    match arg {
      Argument::StringLiteral(str) => {
        if let Some(glob) = str.value.strip_prefix('!') {
          negated_globs.push(glob);
        } else {
          positive_globs.push(str.value.as_str());
        }
      }
      Argument::ArrayExpression(array_expr) => {
        for expr in &array_expr.elements {
          if let ArrayExpressionElement::StringLiteral(str) = expr {
            if let Some(glob) = str.value.strip_prefix('!') {
              negated_globs.push(glob);
            } else {
              positive_globs.push(str.value.as_str());
            }
          }
        }
      }
      _ => {}
    }

    let root = &self.root;
    let dir = Path::new(self.id).parent().unwrap_or_else(|| Path::new(root));
    let dir = if dir.to_slash_lossy() == "" { Path::new(root) } else { dir };

    let negated_globs = negated_globs
      .iter()
      .map(|g| {
        let g = preprocess_glob_expr(g);
        let g = to_absolute_glob(&g, dir, root).unwrap();
        Pattern::new(&g).unwrap()
      })
      .collect::<std::vec::Vec<_>>();

    let is_relative = positive_globs.iter().all(|g| g.starts_with('.'));

    let self_path = self.format_path(Path::new(self.id), Some(dir));

    for glob_expr in positive_globs {
      let processed_glob_expr = preprocess_glob_expr(glob_expr);
      let absolute_glob = to_absolute_glob(&processed_glob_expr, dir, root).unwrap();
      // TODO handle error
      for file in glob(&absolute_glob).unwrap() {
        let file = file.unwrap();
        if negated_globs.iter().any(|g| g.matches_path(&file)) {
          continue;
        }
        let import_path = self.format_path(&file, Some(dir));
        if import_path == self_path {
          continue;
        }
        let key = if is_relative { import_path.clone() } else { self.format_path(&file, None) };
        files.push(FileData { key, import_path });
      }
    }
  }

  fn format_path(&self, path: &Path, relative_to: Option<&Path>) -> String {
    let dir = relative_to.unwrap_or(self.root);
    let path = path.relative(dir).to_slash_lossy().to_string();
    let prefix = if path.starts_with('.') {
      ""
    } else if relative_to.is_some() {
      "./"
    } else {
      "/"
    };
    format!("{prefix}{path}")
  }

  #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
  fn generate_glob_object_expression(
    &mut self,
    files: &[FileData],
    opts: &ImportGlobOptions,
    call_expr_span: Span,
    omit_keys: bool,
    omit_values: bool,
  ) -> Expression<'ast> {
    let properties = files.iter().enumerate().map(|(index, file_data)| {
      let import_path = &file_data.import_path;
      let formatted_file = if let Some(query) = &opts.query {
        let normalized_query = if query == "?raw" {
          query
        } else {
          let file_extension =
            Path::new(&import_path).extension().unwrap_or_default().to_str().unwrap_or_default();
          if !file_extension.is_empty() && self.restore_query_extension {
            &format!("{query}&lang.{file_extension}")
          } else {
            query
          }
        };
        Cow::Owned(format!("{import_path}{normalized_query}"))
      } else {
        Cow::Borrowed(import_path)
      };

      let value = if omit_values {
        self.ast_builder.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal)
      } else if opts.eager.unwrap_or_default() {
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

        self.import_decls.push(Statement::from(
          self.ast_builder.module_declaration_import_declaration(
            SPAN,
            Some(self.ast_builder.vec1(module_specifier)),
            self.ast_builder.string_literal(Span::default(), formatted_file.as_str(), None),
            None,
            NONE,
            ImportOrExportKind::Value,
          ),
        ));

        self.ast_builder.expression_identifier(SPAN, &name)
      } else {
        // import('./dir/bar.js')
        let mut import_expression = self.ast_builder.expression_import(
          SPAN,
          self.ast_builder.expression_string_literal(
            Span::default(),
            formatted_file.as_str(),
            None,
          ),
          self.ast_builder.vec(),
          None,
        );
        // import('./dir/foo.js').then((m) => m.setup)
        if let Some(import) = &opts.import {
          if import != "*" {
            import_expression = self.ast_builder.expression_call(
              SPAN,
              Expression::from(self.ast_builder.member_expression_static(
                SPAN,
                import_expression,
                self.ast_builder.identifier_name(SPAN, "then"),
                false,
              )),
              NONE,
              self.ast_builder.vec1(
                self
                  .ast_builder
                  .expression_arrow_function(
                    SPAN,
                    true,
                    false,
                    NONE,
                    self.ast_builder.formal_parameters(
                      SPAN,
                      FormalParameterKind::ArrowFormalParameters,
                      self.ast_builder.vec1(self.ast_builder.formal_parameter(
                        SPAN,
                        self.ast_builder.vec(),
                        self.ast_builder.binding_pattern(
                          self.ast_builder.binding_pattern_kind_binding_identifier(SPAN, "m"),
                          NONE,
                          false,
                        ),
                        None,
                        false,
                        false,
                      )),
                      NONE,
                    ),
                    NONE,
                    self.ast_builder.function_body(
                      SPAN,
                      self.ast_builder.vec(),
                      self.ast_builder.vec1(self.ast_builder.statement_expression(
                        SPAN,
                        Expression::from(self.ast_builder.member_expression_static(
                          SPAN,
                          self.ast_builder.expression_identifier(SPAN, "m"),
                          self.ast_builder.identifier_name(SPAN, import),
                          false,
                        )),
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
          NONE,
          self.ast_builder.formal_parameters(
            SPAN,
            FormalParameterKind::ArrowFormalParameters,
            self.ast_builder.vec(),
            NONE,
          ),
          NONE,
          self.ast_builder.function_body(
            SPAN,
            self.ast_builder.vec(),
            self.ast_builder.vec1(self.ast_builder.statement_expression(SPAN, import_expression)),
          ),
        )
      };

      (&file_data.key, value)
    });

    if omit_keys {
      let elements = properties.map(|(_, value)| ArrayExpressionElement::from(value));
      let elements = self.ast_builder.vec_from_iter(elements);
      self.ast_builder.expression_array(call_expr_span, elements, None)
    } else {
      let properties = properties.map(|(file, value)| {
        self.ast_builder.object_property_kind_object_property(
          SPAN,
          PropertyKind::Init,
          PropertyKey::from(self.ast_builder.expression_string_literal(
            Span::default(),
            file,
            None,
          )),
          value,
          false,
          false,
          false,
        )
      });
      let properties = self.ast_builder.vec_from_iter(properties);
      self.ast_builder.expression_object(call_expr_span, properties, None)
    }
  }
}

/// hack some syntax that `glob` did not support
/// 1. `**.js` -> `*.js`
fn preprocess_glob_expr(glob_expr: &str) -> String {
  let mut parts = glob_expr.split('/').peekable();
  let mut new_glob_expr = String::with_capacity(glob_expr.len());
  while let Some(part) = parts.next() {
    new_glob_expr.push_str(&part.replace("**.", "*."));
    if parts.peek().is_some() {
      new_glob_expr.push('/');
    }
  }
  new_glob_expr
}

fn to_absolute_glob(glob: &str, dir: &Path, root: &Path) -> anyhow::Result<String> {
  let absolute_glob = if let Some(glob) = glob.strip_prefix('/') {
    root.join(glob)
  } else if glob.starts_with('.') {
    dir.join(glob)
  } else if glob.starts_with("**") {
    // TODO allow this only when pattern is negated to avoid globbing entire fs
    // or consider making it relative to root when it's not negated
    return Ok(glob.to_string());
  } else {
    // https://github.com/rolldown/vite/blob/454c8fff9f7115ed29281c2d927366280508a0ab/packages/vite/src/node/plugins/importMetaGlob.ts#L563-L569
    // Needs to investigate if oxc resolver support this pattern
    return Err(anyhow::format_err!("Invalid glob pattern: {}", glob));
  };
  Ok(absolute_glob.to_slash_lossy().to_string())
}
