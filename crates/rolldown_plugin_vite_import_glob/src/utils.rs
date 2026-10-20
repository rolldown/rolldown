use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxc::ast::ast::{
  self, Argument, ArrayExpressionElement, Expression, ObjectPropertyKind, PropertyKey, PropertyKind,
};
use oxc::ast_visit::{VisitJs, walk_js};
use oxc::str::JSStr;
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_plugin::{LogWithoutPlugin, PluginContext};
use rolldown_plugin_utils::{
  constants::{ViteImportGlob, ViteImportGlobValue},
  to_string_literal,
};
use rolldown_std_utils::relative_path_to_slash;
use string_wizard::MagicString;
use sugar_path::SugarPath;

use crate::matcher::GlobMatcher;

pub struct GlobImportVisit<'a> {
  pub ctx: &'a PluginContext,
  pub id: &'a str,
  pub root: &'a PathBuf,
  pub restore_query_extension: bool,
  pub current: usize,
  pub code: &'a str,
  pub magic_string: Option<MagicString<'a>>,
  pub import_decls: Vec<String>,
  pub errors: Vec<anyhow::Error>,
  pub is_dev_mode: bool,
  pub matchers: Vec<GlobMatcher>,
}

impl<'ast> VisitJs<'ast> for GlobImportVisit<'_> {
  fn visit_program(&mut self, it: &ast::Program<'ast>) {
    walk_js::walk_program(self, it);
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
    walk_js::walk_expression(self, expr);
  }
}

#[derive(Debug, Default)]
pub struct ImportGlobOptions {
  eager: bool,
  exhaustive: bool,
  base: Option<String>,
  query: Option<String>,
  import: Option<String>,
  case_sensitive: Option<bool>,
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

  /// `path` is literal, so its glob syntax is escaped, like vite's `globSafeResolvedPath`.
  fn to_absolute_glob(&self) -> String {
    let mut glob = String::with_capacity(self.path.len() + self.glob.len());
    for char in self.path.chars() {
      if matches!(char, '\\' | '*' | '?' | '[' | ']' | '{' | '}') {
        glob.push('\\');
      }
      glob.push(char);
    }
    glob.push_str(self.glob);
    glob
  }

  fn find_glob_syntax(path: &str) -> usize {
    let mut last_slash = 0;
    for (i, b) in path.as_bytes().iter().enumerate() {
      if *b == b'/' {
        last_slash = i;
      } else if b"\\*?[]{}".contains(b) {
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
      let p = path[path.len() - 1 - num_equal];
      let g = glob[glob.len() - 1 - num_equal];
      if p != g {
        break;
      }
      num_equal += 1;
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
      Expression::ImportMeta(_) if mem_expr.property.name == "glob" => {
        let mut files: Vec<ImportGlobFileData> = vec![];
        let mut options = ImportGlobOptions::default();

        // import.meta.glob(['./dir/*.js'], { import: 'setup' })
        if let Some(arg) = call_expr.arguments.get(1)
          && let Err(err) = Self::update_options(arg, &mut options)
        {
          self.errors.push(err);
          return true;
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

      let formatted_file = to_string_literal(&formatted_file);
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

        self.import_decls.push(format!("import {module_specifier} from {formatted_file};"));

        Cow::Owned(name)
      } else {
        // () => import('./dir/bar.js') or () => import('./dir/foo.js').then((m) => m.setup)
        Cow::Owned(match options.import.as_deref() {
          Some(import) if import != "*" => {
            format!("() => import({formatted_file}).then((m) => m[{}])", to_string_literal(import))
          }
          _ => format!("() => import({formatted_file})"),
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
          .map(|(file, value)| format!("{}: {value}", to_string_literal(file)))
          .collect::<Vec<_>>()
          .join(",")
      ),
      ImportGlobOmitType::None => format!(
        "/* #__PURE__ */ Object.assign({{{}{line_breaks}}})",
        properties
          .map(|(file, value)| format!("{}: {value}", to_string_literal(file)))
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
    dir: &str,
    root: &str,
    base: Option<&str>,
  ) -> Option<PathWithGlob<'a>> {
    let dir = if let Some(base) = base {
      if let Some(base) = base.strip_prefix('/') {
        path_posix::join(&[root, base])
      } else {
        path_posix::join(&[dir, base])
      }
    } else {
      Cow::Borrowed(dir)
    };
    let absolute_glob = if let Some(glob) = glob.strip_prefix('/') {
      path_posix::join(&[root, glob])
    } else if glob.starts_with("**") {
      path_posix::join(&[root, glob])
    } else if glob.starts_with("./") || glob.starts_with("../") {
      path_posix::join(&[&dir, glob])
    } else {
      let is_sub_imports_pattern = glob.starts_with('#') && glob.contains('*');
      let mut custom = rolldown_plugin::CustomField::new();
      custom.insert(ViteImportGlob, ViteImportGlobValue(is_sub_imports_pattern));
      let future = self.ctx.resolve(
        glob,
        Some(self.id),
        Some(rolldown_plugin::PluginContextResolveOptions {
          custom: Arc::new(custom),
          ..Default::default()
        }),
      );

      let resolved_id =
        rolldown_utils::futures::block_on(future).ok().and_then(Result::ok).map(|resolved| {
          path_posix::normalize(&rolldown_utils::pattern_filter::normalize_path(
            resolved.id.as_str(),
          ))
          .into_owned()
        });

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
    Some(PathWithGlob::new(absolute_glob.into_owned(), glob))
  }

  fn relative_path(&self, path: &Path, to: Option<&Path>) -> String {
    let path = relative_path_to_slash(path, to.unwrap_or(self.root));
    if path.starts_with("./") || path.starts_with("../") {
      path
    } else {
      let prefix = if to.is_none() { "/" } else { "./" };
      format!("{prefix}{path}")
    }
  }

  fn get_common_base(&self, globs: &[PathWithGlob]) -> Cow<'_, str> {
    let Some((head, tail)) = globs.split_first() else {
      return self.root.to_slash_lossy();
    };

    let first = head.path.as_bytes();
    let mut end = first.len();
    for PathWithGlob { path, .. } in tail {
      let bytes = path.as_bytes();
      let max_len = end.min(bytes.len());

      let mut i = 0;
      let mut last_slash = 0;
      while i < max_len && first[i] == bytes[i] {
        if first[i] == b'/' {
          last_slash = i;
        }
        i += 1;
      }

      // If the walk finished at `max_len` (not a byte mismatch), `i` itself
      // may already be a deeper boundary than any separator we recorded —
      // e.g. when one path fully matched the other up to a segment split.
      if i == max_len
        && (i == first.len() || first[i] == b'/')
        && (i == bytes.len() || bytes[i] == b'/')
      {
        last_slash = i;
      }

      end = last_slash;
      if end == 0 {
        break;
      }
    }

    if end == 0 { self.root.to_slash_lossy() } else { Cow::Owned(head.path[..end].to_string()) }
  }

  fn eval_glob_expr(
    &mut self,
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

    let dir_slash = dir.to_slash_lossy();
    let root_slash = root.to_slash_lossy();

    let mut is_relative = true;
    let mut negated_globs = vec![];
    let mut positive_globs = vec![];

    // A glob with a lone surrogate matches no file path, so it is skipped.
    let mut values = vec![];
    match arg {
      Argument::StringLiteral(str) => values.extend(str.value.as_str()),
      Argument::TemplateLiteral(lit) => {
        values.extend(lit.single_quasi().and_then(JSStr::as_str));
      }
      Argument::ArrayExpression(array_expr) => {
        for expr in &array_expr.elements {
          match expr {
            ArrayExpressionElement::StringLiteral(str) => values.extend(str.value.as_str()),
            ArrayExpressionElement::TemplateLiteral(lit) => {
              values.extend(lit.single_quasi().and_then(JSStr::as_str));
            }
            _ => {}
          }
        }
      }
      _ => {}
    }

    for value in values {
      if let Some(glob) = value.strip_prefix('!') {
        negated_globs.push(self.to_absolute_glob(
          glob,
          &dir_slash,
          &root_slash,
          options.base.as_deref(),
        )?);
      } else {
        positive_globs.push(self.to_absolute_glob(
          value,
          &dir_slash,
          &root_slash,
          options.base.as_deref(),
        )?);
        if !value.starts_with('.') {
          is_relative = false;
        }
      }
    }

    if negated_globs.is_empty() && positive_globs.is_empty() {
      return Some(());
    }

    let case_sensitive = options.case_sensitive.unwrap_or(true);
    // `fast_glob` has no case-insensitive flag, so we approximate it by lowercasing
    // both the glob (base + pattern) and the candidate path before matching. This
    // matches picomatch's `nocase` for ASCII; it may diverge on character-class
    // ranges (e.g. `[A-Z]`) or non-ASCII case folding, which is acceptable here.
    let insensitive_globs = (!case_sensitive).then(|| {
      let lower = |globs: &[PathWithGlob]| {
        globs
          .iter()
          .map(|glob| (glob.path.to_lowercase(), glob.glob.to_lowercase()))
          .collect::<Vec<_>>()
      };
      (lower(&negated_globs), lower(&positive_globs))
    });

    if is_virtual_module && is_relative && options.base.as_ref().is_none() {
      self.errors.push(anyhow::anyhow!("In virtual modules, all globs must start with '/'"));
      return None;
    }

    // Nothing can match, and the common base would be the whole root.
    if positive_globs.is_empty() {
      return Some(());
    }

    let common = self.get_common_base(&positive_globs).into_owned();
    let common_path = Path::new(&common);

    if self.is_dev_mode {
      self.watch_walk_root(common_path);
      let to_absolute_globs =
        |globs: &[PathWithGlob]| globs.iter().map(PathWithGlob::to_absolute_glob).collect();
      // A glob that globstar rejects gets no matcher. The walk does not fail on it either.
      if let Ok(matcher) = GlobMatcher::new(
        to_absolute_globs(&positive_globs),
        to_absolute_globs(&negated_globs),
        options.exhaustive,
        case_sensitive,
      ) {
        self.matchers.push(matcher);
      }
    }

    let entries = walkdir::WalkDir::new(common_path)
      .follow_links(true)
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

    for entry in entries {
      let file = entry.path();
      let path = file.to_slash_lossy();

      // Skip the file itself if it matches the glob pattern, to avoid self-importing.
      if self.id == path {
        continue;
      }

      let matches = match &insensitive_globs {
        None => {
          let matches_rule = |v: &PathWithGlob| -> bool {
            path.strip_prefix(&v.path).is_some_and(|path| fast_glob::glob_match(v.glob, path))
          };
          !negated_globs.iter().any(matches_rule) && positive_globs.iter().any(matches_rule)
        }
        Some((negated, positive)) => {
          let path = path.to_lowercase();
          let matches_rule = |(base, glob): &(String, String)| -> bool {
            path.strip_prefix(base).is_some_and(|path| fast_glob::glob_match(glob, path))
          };
          !negated.iter().any(matches_rule) && positive.iter().any(matches_rule)
        }
      };

      if !matches {
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

      let import_path = self.relative_path(file, Some(dir));
      let file_path = if let Some(base) = &options.base {
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

  /// A directory is watched with everything below it, so the walk root covers all the walk
  /// reads. The watcher can only wait for a missing path whose parent exists.
  fn watch_walk_root(&self, walk_root: &Path) {
    // The id of a virtual module can make it relative.
    if walk_root.is_absolute() {
      let path =
        walk_root.ancestors().take_while(|path| !path.exists()).last().unwrap_or(walk_root);
      self.ctx.add_watch_file(&path.to_slash_lossy());
    }
  }

  /// `base`, `import` and `query` end up in paths, export names and import specifiers, which are
  /// UTF-8, so a value with a lone surrogate is an error.
  fn update_options(arg: &Argument, options: &mut ImportGlobOptions) -> anyhow::Result<()> {
    let Argument::ObjectExpression(obj) = arg else {
      return Ok(());
    };

    for prop in &obj.properties {
      let ObjectPropertyKind::ObjectProperty(p) = prop else {
        continue;
      };

      let PropertyKind::Init = p.kind else {
        continue;
      };

      // A key with a lone surrogate names no option, so it is ignored like any unknown key.
      let key = match &p.key {
        PropertyKey::StringLiteral(str) => str.value.as_str(),
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
        _ => continue,
      };

      match key {
        Some("base") => match &p.value {
          Expression::StringLiteral(str) if !str.value.is_empty() => {
            options.base = Some(option_str(str.value)?.to_string());
          }
          Expression::TemplateLiteral(str)
            if str.is_no_substitution_template() && !str.quasis[0].value.raw.is_empty() =>
          {
            options.base = Some(str.quasis[0].value.raw.as_str().to_string());
          }
          _ => {}
        },
        Some("import") => {
          if let Expression::StringLiteral(str) = &p.value {
            options.import = Some(option_str(str.value)?.to_string());
          }
        }
        Some("eager") => {
          if let Expression::BooleanLiteral(bool) = &p.value {
            options.eager = bool.value;
          }
        }
        Some("exhaustive") => {
          if let Expression::BooleanLiteral(bool) = &p.value {
            options.exhaustive = bool.value;
          }
        }
        Some("caseSensitive") => {
          if let Expression::BooleanLiteral(bool) = &p.value {
            options.case_sensitive = Some(bool.value);
          }
        }
        Some("query") => match &p.value {
          Expression::StringLiteral(str) => {
            let query = option_str(str.value)?;
            options.query =
              Some(if query.starts_with('?') { query.to_string() } else { format!("?{query}") });
          }
          Expression::ObjectExpression(expr) => {
            let mut query_string = String::from("?");
            for prop in &expr.properties {
              let ObjectPropertyKind::ObjectProperty(p) = prop else { continue };

              let key = match &p.key {
                PropertyKey::StringLiteral(key) => option_str(key.value)?,
                PropertyKey::StaticIdentifier(ident) => ident.name.as_str(),
                _ => continue,
              };

              let value = match &p.value {
                Expression::StringLiteral(v) => option_str(v.value)?,
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
    Ok(())
  }
}

fn option_str(value: JSStr<'_>) -> anyhow::Result<&str> {
  value
    .as_str()
    .ok_or_else(|| anyhow::anyhow!("`import.meta.glob()` options cannot contain lone surrogates."))
}

#[cfg(test)]
mod tests {
  use oxc::{allocator::Allocator, parser::Parser, span::SourceType};

  use super::*;

  fn options_error(options: &str) -> Option<String> {
    let allocator = Allocator::default();
    let code = format!("f({options})");
    let expr = Parser::new(&allocator, &code, SourceType::default()).parse_expression().unwrap();
    let Expression::CallExpression(call) = expr else { unreachable!() };
    GlobImportVisit::update_options(&call.arguments[0], &mut ImportGlobOptions::default())
      .err()
      .map(|err| err.to_string())
  }

  #[test]
  fn rejects_options_with_lone_surrogates() {
    for options in [
      "{ base: './\\uD800' }",
      "{ import: '\\uD800' }",
      "{ query: '\\uD800' }",
      "{ query: { '\\uD800': true } }",
      "{ query: { raw: '\\uD800' } }",
    ] {
      assert_eq!(
        options_error(options).as_deref(),
        Some("`import.meta.glob()` options cannot contain lone surrogates."),
        "{options}"
      );
    }
    // A key with a lone surrogate names no option.
    assert_eq!(options_error("{ '\\uD800': 'x', import: 'default' }"), None);
  }
}
