use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use glob::Pattern;
use oxc::ast::NONE;
use oxc::ast::ast::{
  Argument, ArrayExpressionElement, Expression, FormalParameterKind, ImportOrExportKind,
  NumberBase, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{SPAN, Span};
use rolldown_ecmascript_utils::ExpressionExt;
use sugar_path::SugarPath;

pub struct GlobImportVisit<'ast, 'a> {
  pub id: Cow<'a, str>,
  pub root: &'a PathBuf,
  pub ast_builder: oxc::ast::AstBuilder<'ast>,
  pub restore_query_extension: bool,
  pub current: usize,
  pub import_decls: oxc::allocator::Vec<'ast, Statement<'ast>>,
}

impl<'ast> VisitMut<'ast> for GlobImportVisit<'ast, '_> {
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    self.transform_glob_import(expr, ImportGlobOmitType::None);
    walk_mut::walk_expression(self, expr);
  }
}

#[derive(Debug, Default)]
pub struct ImportGlobOptions {
  eager: bool,
  query: Option<String>,
  import: Option<String>,
}

struct ImportGlobFileData {
  file_path: Option<String>,
  import_path: String,
}

#[derive(Clone, Copy)]
enum ImportGlobOmitType {
  Keys,
  Values,
  None,
}

impl<'ast> GlobImportVisit<'ast, '_> {
  fn transform_glob_import(&mut self, expr: &mut Expression<'ast>, omit_type: ImportGlobOmitType) {
    let Some(call_expr) = expr.as_call_expression_mut() else { return };
    let Some(mem_expr) = call_expr.callee.as_static_member_expr_mut() else { return };

    match &mem_expr.object {
      Expression::Identifier(id)
        if matches!(omit_type, ImportGlobOmitType::None) && id.name == "Object" =>
      {
        let omit_type = match mem_expr.property.name.as_str() {
          "keys" => ImportGlobOmitType::Values,
          "values" => ImportGlobOmitType::Keys,
          _ => return,
        };
        let [arg] = call_expr.arguments.as_mut_slice() else { return };
        let Some(arg_expr) = arg.as_expression_mut() else { return };
        self.transform_glob_import(arg_expr, omit_type);
      }
      Expression::MetaProperty(p)
        if mem_expr.property.name == "glob"
          && p.meta.name == "import"
          && p.property.name == "meta" =>
      {
        let mut files: Vec<ImportGlobFileData> = vec![];
        let mut options = ImportGlobOptions::default();

        // import.meta.glob('./dir/*.js')
        if let Some(arg) = call_expr.arguments.first() {
          self.eval_glob_expr(arg, &mut files);
        }

        // import.meta.glob(['./dir/*.js'], { import: 'setup' })
        if let Some(arg) = call_expr.arguments.get(1) {
          Self::update_options(arg, &mut options);
        }

        // {
        //   './dir/ind.js': __glob__0_0_,
        //   './dir/foo.js': () => import('./dir/foo.js'),
        //   './dir/bar.js': () => import('./dir/bar.js?raw').then((m) => m.setup),
        // }
        *expr = self.generate_glob_object_expression(&files, &options, omit_type, call_expr.span);

        self.current += 1;
      }
      _ => {}
    }
  }

  #[allow(clippy::too_many_lines)]
  fn generate_glob_object_expression(
    &mut self,
    files: &[ImportGlobFileData],
    options: &ImportGlobOptions,
    omit_type: ImportGlobOmitType,
    span: Span,
  ) -> Expression<'ast> {
    let properties = files.iter().enumerate().map(|(index, file_data)| {
      let import_path = &file_data.import_path;
      let formatted_file = if let Some(query) = &options.query {
        let normalized_query = if query != "?raw" && self.restore_query_extension {
          let path = Path::new(&import_path);
          let extension = path.extension().and_then(|p| p.to_str()).unwrap_or_default();
          &format!("{query}&lang.{extension}")
        } else {
          query
        };
        Cow::Owned(format!("{import_path}{normalized_query}"))
      } else {
        Cow::Borrowed(import_path)
      };

      let value = if matches!(omit_type, ImportGlobOmitType::Values) {
        self.ast_builder.expression_numeric_literal(SPAN, 0.0, None, NumberBase::Decimal)
      } else if options.eager {
        // import * as __import_glob__0_0_ from './dir/foo.js'
        // const modules = {
        //   './dir/foo.js': __import_glob__0_0_,
        // }
        let name = format!(
          "__import_glob__{}_{}_",
          itoa::Buffer::new().format(self.current),
          itoa::Buffer::new().format(index)
        );

        let module_specifier = match options.import.as_deref() {
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
          None,
          None,
        );

        // import('./dir/foo.js').then((m) => m.setup)
        if let Some(import) = &options.import {
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

      if let Some(file_path) = &file_data.file_path {
        (file_path, value)
      } else {
        (import_path, value)
      }
    });

    if matches!(omit_type, ImportGlobOmitType::Keys) {
      let elements = properties.map(|(_, value)| ArrayExpressionElement::from(value));
      let elements = self.ast_builder.vec_from_iter(elements);
      self.ast_builder.expression_array(span, elements)
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
      self.ast_builder.expression_object(span, properties)
    }
  }
}

impl GlobImportVisit<'_, '_> {
  fn is_virtual_module(&self) -> bool {
    // https://vite.dev/guide/api-plugin.html#virtual-modules-convention
    self.id.starts_with("virtual:") || self.id.starts_with('\0') || !self.id.contains('/')
  }

  fn to_absolute_glob<'a>(&self, glob: &'a str, dir: &Path, root: &Path) -> Cow<'a, str> {
    // hack some syntax that `glob` did not support
    // 1. `**.js` -> `*.js`
    let index = glob.rfind('/').unwrap_or(0);
    let glob = if glob[index..].contains("**.") {
      let mut result = String::with_capacity(glob.len());
      if index != 0 {
        result.push_str(&glob[..index]);
      }
      result.push_str(&glob[index..].replace("**.", "*."));
      Cow::Owned(result)
    } else {
      Cow::Borrowed(glob)
    };

    let absolute_glob = if let Some(glob) = glob.strip_prefix('/') {
      root.join(glob)
    } else if glob.starts_with('.') {
      dir.join(glob.as_ref())
    } else if glob.starts_with("**") {
      return glob;
    } else {
      // https://github.com/rolldown/vite/blob/454c8fff9f7115ed29281c2d927366280508a0ab/packages/vite/src/node/plugins/importMetaGlob.ts#L563-L569
      // TODO: Needs to investigate if oxc resolver support this pattern
      panic!(
        "Invalid glob pattern: {glob} (resolved: '{}'), it must start with '/' or './'.",
        self.id
      );
    };
    Cow::Owned(absolute_glob.to_slash_lossy().to_string())
  }

  fn relative_path(&self, path: &Path, to: Option<&Path>) -> String {
    let path = path.relative(to.unwrap_or(self.root));
    let path = path.to_slash_lossy();
    if path.starts_with('.') {
      path.to_string()
    } else {
      let prefix = if to.is_none() { "/" } else { "./" };
      format!("{prefix}{path}")
    }
  }

  fn eval_glob_expr(&self, arg: &Argument, files: &mut Vec<ImportGlobFileData>) {
    let root = Path::new(self.root);
    let is_virtual_module = self.is_virtual_module();

    let dir = if is_virtual_module {
      root
    } else {
      let id = Path::new(self.id.as_ref());
      id.parent().unwrap_or(root)
    };

    let mut is_relative = true;
    let mut negated_globs = vec![];
    let mut positive_globs = vec![];

    match arg {
      Argument::StringLiteral(str) => {
        if let Some(glob) = str.value.strip_prefix('!') {
          let glob = self.to_absolute_glob(glob, dir, root);
          negated_globs.push(Pattern::new(&glob).unwrap());
        } else {
          positive_globs.push(self.to_absolute_glob(&str.value, dir, root));
          if !str.value.starts_with('.') {
            is_relative = false;
          }
        }
      }
      Argument::ArrayExpression(array_expr) => {
        for expr in &array_expr.elements {
          if let ArrayExpressionElement::StringLiteral(str) = expr {
            if let Some(glob) = str.value.strip_prefix('!') {
              let glob = self.to_absolute_glob(glob, dir, root);
              negated_globs.push(Pattern::new(&glob).unwrap());
            } else {
              positive_globs.push(self.to_absolute_glob(&str.value, dir, root));
              if !str.value.starts_with('.') {
                is_relative = false;
              }
            }
          }
        }
      }
      _ => {}
    }

    assert!(
      !(is_virtual_module && is_relative),
      "In virtual modules, all globs must start with '/'"
    );

    let self_path = self.relative_path(Path::new(self.id.as_ref()), Some(dir));
    for glob_expr in positive_globs {
      for file in glob::glob(&glob_expr).unwrap() {
        let file = file.unwrap();
        if negated_globs.iter().any(|g| g.matches_path(&file)) {
          continue;
        }

        let file_path = self.relative_path(&file, None);
        if is_virtual_module {
          let path = if file_path.starts_with('/') { file_path } else { format!("/{file_path}") };
          files.push(ImportGlobFileData { file_path: None, import_path: path });
          continue;
        }

        let import_path = self.relative_path(&file, Some(dir));
        if self_path == import_path {
          continue;
        }

        let file_path = if is_relative { None } else { Some(file_path) };

        files.push(ImportGlobFileData { file_path, import_path });
      }
    }
  }

  fn update_options(arg: &Argument, options: &mut ImportGlobOptions) {
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
            options.import = Some(str.value.as_str().to_string());
          }
        }
        "eager" => {
          if let Expression::BooleanLiteral(bool) = &p.value {
            options.eager = bool.value;
          }
        }
        "query" => match &p.value {
          Expression::StringLiteral(str) => {
            options.query = if str.value.starts_with('?') {
              Some(str.value.to_string())
            } else {
              Some(format!("?{}", str.value))
            }
          }
          Expression::ObjectExpression(expr) => {
            let mut query_string = String::from("?");
            for prop in &expr.properties {
              let ObjectPropertyKind::ObjectProperty(p) = prop else { continue };

              let key = match &p.key {
                PropertyKey::StringLiteral(key) => key.value,
                PropertyKey::StaticIdentifier(ident) => ident.name,
                _ => continue,
              };

              let value = match &p.value {
                Expression::StringLiteral(v) => v.value.as_str(),
                Expression::BooleanLiteral(v) => {
                  if v.value {
                    "true"
                  } else {
                    "false"
                  }
                }
                Expression::NumericLiteral(v) => &v.value.to_string(),
                Expression::NullLiteral(_) => "null",
                _ => continue,
              };

              if query_string.len() != 1 {
                query_string.push('&');
              }
              write!(query_string, "{key}={value}").unwrap();
            }

            if query_string.len() != 1 {
              options.query = Some(query_string);
            }
          }
          _ => {}
        },
        _ => {}
      }
    }
  }
}
