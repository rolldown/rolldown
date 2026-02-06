use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::sync::Arc;

use oxc::ast::ast::{
  self, Argument, ArrayExpressionElement, Expression, ObjectPropertyKind, PropertyKey, PropertyKind,
};
use oxc::ast_visit::{Visit, walk};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_plugin_utils::constants::{ViteImportGlob, ViteImportGlobValue};
use string_wizard::MagicString;
use sugar_path::SugarPath;

pub struct GlobImportVisit<'a> {
  pub ctx: &'a PluginContext,
  pub id: &'a str,
  pub root: &'a PathBuf,
  pub restore_query_extension: bool,
  pub current: usize,
  pub code: &'a str,
  pub magic_string: Option<MagicString<'a>>,
  pub import_decls: Vec<String>,
}

impl<'ast> Visit<'ast> for GlobImportVisit<'_> {
  fn visit_program(&mut self, it: &ast::Program<'ast>) {
    walk::walk_program(self, it);
    if !self.import_decls.is_empty() {
      self
        .magic_string
        .get_or_insert_with(|| MagicString::new(self.code))
        .prepend(self.import_decls.join("\n"));
    }
  }
  fn visit_expression(&mut self, expr: &Expression<'ast>) {
    if self.transform_glob_import(expr, ImportGlobOmitType::None) {
      return;
    }
    walk::walk_expression(self, expr);
  }
}

#[derive(Debug, Default)]
pub struct ImportGlobOptions {
  eager: bool,
  exhaustive: bool,
  base: Option<String>,
  query: Option<String>,
  import: Option<String>,
}

struct ImportGlobFileData {
  file_path: Option<String>,
  import_path: String,
}

#[derive(Debug)]
struct PathWithGlob<'a> {
  pub path: String,
  pub glob: &'a str,
}

impl<'a> PathWithGlob<'a> {
  fn new(mut path: String, glob: &'a str) -> Self {
    let j = Self::split_path_and_glob_inner(&path, glob);
    let i = Self::find_glob_syntax(&glob[glob.len() - j..]);
    path.truncate(path.len() - i);
    Self { path, glob: &glob[glob.len() - i..] }
  }

  fn find_glob_syntax(path: &str) -> usize {
    let mut last_slash = 0;
    for (i, b) in path.as_bytes().iter().enumerate() {
      if *b == b'/' {
        last_slash = i;
      } else if [b'*', b'?', b'[', b']', b'{', b'}'].contains(b) {
        return path.len() - last_slash;
      }
    }
    path.len() - last_slash
  }

  fn split_path_and_glob_inner(path: &str, glob: &str) -> usize {
    let path = path.as_bytes();
    let glob = glob.as_bytes();

    let mut num_equal = 0;
    let max_equal = path.len().min(glob.len());
    while num_equal < max_equal {
      let r_ch = path[path.len() - 1 - num_equal];
      let g_ch = glob[glob.len() - 1 - num_equal];

      if r_ch == g_ch || (g_ch == b'/' && r_ch == MAIN_SEPARATOR as u8) {
        num_equal += 1;
      } else {
        break;
      }
    }

    num_equal
  }
}

#[derive(Clone, Copy, Debug)]
enum ImportGlobOmitType {
  Keys,
  Values,
  None,
}

impl<'ast> GlobImportVisit<'_> {
  fn transform_glob_import(
    &mut self,
    expr: &Expression<'ast>,
    omit_type: ImportGlobOmitType,
  ) -> bool {
    let Some(call_expr) = expr.as_call_expression() else { return false };
    let ast::Expression::StaticMemberExpression(ref mem_expr) = call_expr.callee else {
      return false;
    };

    match &mem_expr.object {
      Expression::Identifier(id)
        if matches!(omit_type, ImportGlobOmitType::None) && id.name == "Object" =>
      {
        let omit_type = match mem_expr.property.name.as_str() {
          "keys" => ImportGlobOmitType::Values,
          "values" => ImportGlobOmitType::Keys,
          _ => return false,
        };
        let [arg] = call_expr.arguments.as_slice() else { return false };
        let Some(arg_expr) = arg.as_expression() else { return false };
        self.transform_glob_import(arg_expr, omit_type)
      }
      Expression::MetaProperty(p)
        if mem_expr.property.name == "glob"
          && p.meta.name == "import"
          && p.property.name == "meta" =>
      {
        let mut files: Vec<ImportGlobFileData> = vec![];
        let mut options = ImportGlobOptions::default();

        // import.meta.glob(['./dir/*.js'], { import: 'setup' })
        if let Some(arg) = call_expr.arguments.get(1) {
          Self::update_options(arg, &mut options);
        }

        // import.meta.glob('./dir/*.js')
        let Some(arg) = call_expr.arguments.first() else { return true };

        // {
        //   './dir/ind.js': __glob__0_0_,
        //   './dir/foo.js': () => import('./dir/foo.js'),
        //   './dir/bar.js': () => import('./dir/bar.js?raw').then((m) => m.setup),
        // }
        if self.eval_glob_expr(arg, &mut files, &options).is_some() {
          self.generate_glob_object_expression(&files, &options, omit_type, call_expr.span);
        }

        self.current += 1;
        true
      }
      _ => false,
    }
  }

  fn generate_glob_object_expression(
    &mut self,
    files: &[ImportGlobFileData],
    options: &ImportGlobOptions,
    omit_type: ImportGlobOmitType,
    span: oxc::span::Span,
  ) {
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

      let value: Cow<'_, str> = if matches!(omit_type, ImportGlobOmitType::Values) {
        Cow::Borrowed("0")
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
          Some("*") | None => {
            format!("* as {name}")
          }
          Some(import) => format!("{{ {import} as {name} }}"),
        };

        self.import_decls.push(format!("import {module_specifier} from \"{formatted_file}\";"));

        Cow::Owned(name)
      } else {
        // () => import('./dir/bar.js') or () => import('./dir/foo.js').then((m) => m.setup)
        Cow::Owned(match options.import.as_deref() {
          Some(import) if import != "*" => {
            format!("() => import(\"{formatted_file}\").then((m) => m[\"{import}\"])")
          }
          _ => format!("() => import(\"{formatted_file}\")"),
        })
      };

      if let Some(file_path) = &file_data.file_path {
        (file_path, value)
      } else {
        (import_path, value)
      }
    });

    // Preserve line breaks from original code for sourcemap alignment
    let line_breaks = "\n".repeat(span.source_text(self.code).matches('\n').count());
    let replacement = match omit_type {
      ImportGlobOmitType::Keys => {
        format!(
          "[{}{line_breaks}]",
          properties.map(|(_, value)| value).collect::<Vec<_>>().join(",")
        )
      }
      ImportGlobOmitType::Values => format!(
        "{{{}{line_breaks}}}",
        properties
          .map(|(file, value)| format!("\"{file}\": {value}"))
          .collect::<Vec<_>>()
          .join(",")
      ),
      ImportGlobOmitType::None => format!(
        "/* #__PURE__ */ Object.assign({{{}{line_breaks}}})",
        properties
          .map(|(file, value)| format!("\"{file}\": {value}"))
          .collect::<Vec<_>>()
          .join(",")
      ),
    };

    self
      .magic_string
      .get_or_insert_with(|| string_wizard::MagicString::new(self.code))
      .update(span.start, span.end, replacement)
      .expect("update should not fail in import glob plugin");
  }
}

impl GlobImportVisit<'_> {
  fn is_virtual_module(&self) -> bool {
    // https://vite.dev/guide/api-plugin.html#virtual-modules-convention
    self.id.starts_with("virtual:") || self.id.starts_with('\0') || !self.id.contains('/')
  }

  fn to_absolute_glob<'a>(
    &self,
    glob: &'a str,
    dir: &Path,
    root: &Path,
    base: Option<&str>,
  ) -> Option<PathWithGlob<'a>> {
    let dir = if let Some(base) = base {
      if let Some(base) = base.strip_prefix('/') { root.join(base) } else { dir.join(base) }
    } else {
      dir.to_path_buf()
    };
    let absolute_glob = if let Some(glob) = glob.strip_prefix('/') {
      root.join(glob)
    } else if glob.starts_with("**") {
      root.join(glob)
    } else if glob.starts_with("./") || glob.starts_with("../") {
      dir.join(glob)
    } else {
      let is_sub_imports_pattern = glob.starts_with('#') && glob.contains('*');
      let future = self.ctx.resolve(
        glob,
        Some(self.id),
        is_sub_imports_pattern.then(|| {
          let custom = Arc::new(rolldown_plugin::CustomField::new());
          custom.insert(ViteImportGlob, ViteImportGlobValue(true));
          rolldown_plugin::PluginContextResolveOptions { custom, ..Default::default() }
        }),
      );

      let resolved_id = rolldown_utils::futures::block_on(future)
        .ok()
        .and_then(Result::ok)
        .map(|resolved| resolved.id.to_string());

      if let Some(ref id) = resolved_id
        && Path::new(id.as_str()).is_absolute()
      {
        return Some(PathWithGlob::new(id.clone(), glob));
      }

      self.ctx.warn(LogWithoutPlugin {
        message: format!(
          "Invalid glob pattern: `{glob}`{} in file '{}'. Glob patterns must start with:\n  • '/' for absolute paths from project root\n  • './' or '../' for relative paths\n  • '**/' for recursive matching from project root\n  • '#' for subpath imports (with '*' wildcard)",
         resolved_id
              .map(|id| format!(" (resolved: `{id}`)"))
              .unwrap_or_default(),
          self.id.relative(self.root).display()
        ),
        ..Default::default()
      });

      return None;
    };
    Some(PathWithGlob::new(absolute_glob.normalize().to_string_lossy().into_owned(), glob))
  }

  fn relative_path(&self, path: &Path, to: Option<&Path>) -> String {
    let path = path.relative(to.unwrap_or(self.root));
    let path = path.to_slash_lossy();
    if path.starts_with("./") || path.starts_with("../") {
      path.to_string()
    } else {
      let prefix = if to.is_none() { "/" } else { "./" };
      format!("{prefix}{path}")
    }
  }

  fn get_common_base(&self, globs: &[PathWithGlob]) -> Cow<'_, str> {
    if globs.is_empty() {
      return self.root.to_string_lossy();
    }

    let first = globs[0].path.as_bytes();
    let mut end = first.len();
    for PathWithGlob { path, .. } in &globs[1..] {
      let bytes = path.as_bytes();
      let max_len = end.min(bytes.len());

      let mut i = 0;
      while i < max_len && first[i] == bytes[i] {
        i += 1;
      }

      end = i;
      if end == 0 {
        break;
      }
    }

    if end == 0 {
      self.root.to_string_lossy()
    } else {
      Cow::Owned(globs[0].path[..end].to_string())
    }
  }

  fn eval_glob_expr(
    &self,
    arg: &Argument,
    files: &mut Vec<ImportGlobFileData>,
    options: &ImportGlobOptions,
  ) -> Option<()> {
    let root = Path::new(&self.root);
    let is_virtual_module = self.is_virtual_module();

    let dir = if is_virtual_module {
      root
    } else {
      let id = Path::new(self.id);
      id.parent().unwrap_or(root)
    };

    let mut is_relative = true;
    let mut negated_globs = vec![];
    let mut positive_globs = vec![];

    match arg {
      Argument::StringLiteral(str) => {
        if let Some(glob) = str.value.strip_prefix('!') {
          negated_globs.push(self.to_absolute_glob(glob, dir, root, options.base.as_deref())?);
        } else {
          positive_globs.push(self.to_absolute_glob(
            &str.value,
            dir,
            root,
            options.base.as_deref(),
          )?);
          if !str.value.starts_with('.') {
            is_relative = false;
          }
        }
      }
      Argument::ArrayExpression(array_expr) => {
        for expr in &array_expr.elements {
          if let ArrayExpressionElement::StringLiteral(str) = expr {
            if let Some(glob) = str.value.strip_prefix('!') {
              negated_globs.push(self.to_absolute_glob(
                glob,
                dir,
                root,
                options.base.as_deref(),
              )?);
            } else {
              positive_globs.push(self.to_absolute_glob(
                &str.value,
                dir,
                root,
                options.base.as_deref(),
              )?);
              if !str.value.starts_with('.') {
                is_relative = false;
              }
            }
          }
        }
      }
      _ => {}
    }

    if negated_globs.is_empty() && positive_globs.is_empty() {
      return Some(());
    }

    assert!(
      !(is_virtual_module && is_relative && options.base.as_ref().is_none()),
      "In virtual modules, all globs must start with '/'"
    );

    let common = self.get_common_base(&positive_globs);
    let entries = walkdir::WalkDir::new(common.as_ref())
      .sort_by(|a, b| a.file_name().cmp(b.file_name()))
      .into_iter()
      .filter_entry(|entry| {
        options.exhaustive || entry.depth() == 0 || {
          let path = entry.file_name();
          if path.as_encoded_bytes().first() == Some(&b'.') {
            return false;
          }
          path.to_str().is_none_or(|s| s != "node_modules")
        }
      })
      .filter_map(Result::ok)
      .filter(|e| !e.file_type().is_dir());

    let self_path = self.relative_path(Path::new(self.id), Some(dir));

    for entry in entries {
      let file = entry.path();
      let path = file.to_string_lossy();

      let matches_rule = |v: &PathWithGlob| -> bool {
        path.strip_prefix(&v.path).map(|path| fast_glob::glob_match(v.glob, path)).unwrap_or(false)
      };
      if negated_globs.iter().any(matches_rule) || !positive_globs.iter().any(matches_rule) {
        continue;
      }

      let file_path = self.relative_path(file, None);
      if is_virtual_module {
        let import_path =
          if file_path.starts_with('/') { file_path } else { format!("/{file_path}") };
        let file_path = options.base.as_ref().map(|base| {
          self.relative_path(file, Some(&self.root.join(base.strip_prefix('/').unwrap_or(base))))
        });
        files.push(ImportGlobFileData { file_path, import_path });
        continue;
      }

      let mut import_path = self.relative_path(file, Some(dir));
      if self_path == import_path {
        continue;
      }

      let file_path = if let Some(base) = &options.base {
        if base.starts_with('/') {
          import_path = self.relative_path(file, None);
        }
        let base_path = if let Some(base) = base.strip_prefix('/') {
          self.root.join(base)
        } else {
          dir.join(base)
        };
        Some(self.relative_path(file, Some(&base_path)))
      } else if is_relative {
        None
      } else {
        Some(file_path)
      };

      files.push(ImportGlobFileData { file_path, import_path });
    }
    Some(())
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
        "base" => match &p.value {
          Expression::StringLiteral(str) if !str.value.is_empty() => {
            options.base = Some(str.value.as_str().to_string());
          }
          Expression::TemplateLiteral(str)
            if str.is_no_substitution_template() && !str.quasis[0].value.raw.is_empty() =>
          {
            options.base = Some(str.quasis[0].value.raw.as_str().to_string());
          }
          _ => {}
        },
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
        "exhaustive" => {
          if let Expression::BooleanLiteral(bool) = &p.value {
            options.exhaustive = bool.value;
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
                PropertyKey::StringLiteral(key) => key.value.as_str(),
                PropertyKey::StaticIdentifier(ident) => ident.name.as_str(),
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
