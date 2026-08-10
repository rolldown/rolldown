use oxc::allocator::Allocator;
use oxc::ast::ast::Program;
use oxc::codegen::{Codegen, CodegenOptions, IndentChar};
use oxc::parser::Parser;
use oxc::span::{SourceType, Span};
use oxc_sourcemap::SourceMap;

pub struct RuntimeBindingGenerator<'a> {
  #[expect(dead_code)]
  allocator: &'a Allocator,
}

impl<'a> RuntimeBindingGenerator<'a> {
  #[expect(dead_code)]
  pub fn new(allocator: &'a Allocator) -> Self {
    Self { allocator }
  }

  pub fn generate_runtime_binding(
    binding_names: &[String],
    decl_id: usize,
    deps: &[String],
    type_params: &[String],
    children: &[(u32, u32)],
    has_side_effect: bool,
  ) -> String {
    let binding_name = binding_names.first().map(String::as_str).unwrap_or("_");
    let children_str = if children.is_empty() {
      "[]".to_string()
    } else {
      // Flat `[start,end,…]` wire format (not nested pairs). Quote restore uses
      // transform-time `child_literals`, not these bundled elements.
      let pairs: Vec<String> =
        children.iter().flat_map(|(s, e)| [s.to_string(), e.to_string()]).collect();
      format!("[{}]", pairs.join(", "))
    };

    let mut elements =
      vec![format!("{decl_id}"), Self::format_deps_function(deps, type_params), children_str];

    if has_side_effect {
      // Pass the binding so Rolldown treats the call as a live reference and keeps the
      // runtime binding (needed to restore `declare global` / `declare module`).
      elements.push(format!("sideEffect({binding_name})"));
    }

    // Extra declarators without init so multi-binding `const a, b` can be renamed by Rolldown
    // (`var a = [...], b`).
    let mut decl_parts = vec![format!("{} = [{}]", binding_name, elements.join(", "))];
    for extra in binding_names.iter().skip(1) {
      decl_parts.push(extra.clone());
    }
    format!("var {}", decl_parts.join(", "))
  }

  fn format_deps_function(deps: &[String], type_params: &[String]) -> String {
    let params = if type_params.is_empty() { String::new() } else { type_params.join(", ") };

    let deps_str = if deps.is_empty() { String::new() } else { deps.join(", ") };

    format!("({params}) => [{deps_str}]")
  }

  #[expect(dead_code)]
  pub fn generate_code(program: &Program<'a>) -> String {
    Codegen::new().build(program).code
  }
}

pub fn generate_code_from_source(source: &str) -> String {
  let allocator = Allocator::default();
  let source_type = SourceType::d_ts();
  let parser = Parser::new(&allocator, source, source_type);
  let parse_result = parser.parse();

  if parse_result.panicked {
    return source.to_string();
  }

  let options =
    CodegenOptions { indent_char: IndentChar::Space, indent_width: 2, ..Default::default() };

  let codegen_result =
    Codegen::new().with_options(options).with_source_text(source).build(&parse_result.program);

  fix_enum_trailing_punctuation(&fix_enum_indentation(&codegen_result.code))
}

/// Remove trailing commas / post-brace semicolons that oxc adds for const enums.
fn fix_enum_trailing_punctuation(code: &str) -> String {
  let lines: Vec<&str> = code.lines().collect();
  let mut out = Vec::with_capacity(lines.len());
  let mut in_enum = false;
  let mut depth = 0i32;

  for line in lines {
    let trimmed = line.trim();
    if !in_enum
      && (trimmed.contains(" enum ")
        || trimmed.starts_with("enum ")
        || trimmed.starts_with("declare enum ")
        || trimmed.starts_with("declare const enum ")
        || trimmed.starts_with("const enum "))
      && trimmed.contains('{')
    {
      in_enum = true;
      depth = 0;
    }

    if in_enum {
      for ch in trimmed.chars() {
        if ch == '{' {
          depth += 1;
        } else if ch == '}' {
          depth -= 1;
        }
      }

      if depth <= 0 && trimmed.starts_with('}') {
        // oxc emits `};` after enum bodies; strip the semicolon.
        let indent_len = line.len() - line.trim_start().len();
        let indent = &line[..indent_len];
        out.push(format!("{indent}}}"));
        in_enum = false;
        continue;
      }
    }

    out.push(line.to_string());
  }

  let joined = out.join("\n");
  let re = regex::Regex::new(r",(\s*\n\s*})").unwrap();
  re.replace_all(&joined, "$1").to_string()
}

/// Fix oxc_codegen adding extra indentation before `enum` inside namespaces.
/// e.g., `  export   enum F {}` → `  export enum F {}`
/// and `    enum Shadowed2 {}` → `  enum Shadowed2 {}` (when inside a namespace)
fn fix_enum_indentation(code: &str) -> String {
  let re_export_enum = regex::Regex::new(r"(\bexport)\s{2,}(enum\b)").unwrap();
  let mut result = Vec::new();
  let mut in_namespace = false;
  let mut brace_depth = 0;

  for line in code.lines() {
    let fixed = re_export_enum.replace_all(line, "$1 $2").to_string();

    let trimmed = fixed.trim();
    if trimmed.starts_with("declare namespace ") || trimmed.starts_with("namespace ") {
      in_namespace = true;
      brace_depth = 0;
    }

    if in_namespace {
      for ch in trimmed.chars() {
        if ch == '{' {
          brace_depth += 1;
        } else if ch == '}' {
          brace_depth -= 1;
          if brace_depth <= 0 {
            in_namespace = false;
          }
        }
      }

      // oxc over-indents `enum` inside namespaces (4 spaces → 2).
      if trimmed.starts_with("enum ") {
        let indent_len = fixed.len() - fixed.trim_start().len();
        if indent_len >= 4 {
          let new_indent = &fixed[..indent_len - 2];
          result.push(format!("{new_indent}{trimmed}"));
          continue;
        }
      }
    }

    result.push(fixed);
  }
  result.join("\n")
}

pub fn extract_span_text(source: &str, span: Span) -> String {
  if (span.end as usize) <= source.len() && span.start < span.end {
    span.source_text(source).to_string()
  } else {
    String::new()
  }
}

pub fn extract_source_text(source: &str, start: u32, end: u32) -> String {
  extract_span_text(source, Span::new(start, end))
}

pub fn generate_code_with_source_map(
  source: &str,
  source_path: &str,
) -> (String, Option<SourceMap<'static>>) {
  let allocator = Allocator::default();
  let source_type = SourceType::d_ts();
  let parser = Parser::new(&allocator, source, source_type);
  let parse_result = parser.parse();

  if parse_result.panicked {
    return (source.to_string(), None);
  }

  let options =
    CodegenOptions { indent_char: IndentChar::Space, indent_width: 2, ..Default::default() };

  let codegen_result = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: Some(source_path.into()),
      indent_char: options.indent_char,
      indent_width: options.indent_width,
      ..options
    })
    .with_source_text(source)
    .build(&parse_result.program);

  (
    fix_enum_trailing_punctuation(&fix_enum_indentation(&codegen_result.code)),
    codegen_result.map.map(SourceMap::into_owned),
  )
}

pub fn generate_declaration_from_source(source: &str) -> String {
  let source = source.trim_end();

  if source.is_empty() {
    return String::new();
  }

  let leading_comments = extract_leading_comments(source);

  let declaration_without_comments = prepare_declaration_body(source);

  // Preserve multi-line var declarators (oxc_codegen collapses them to one line).
  let result = if should_preserve_var_decl_formatting(&declaration_without_comments) {
    let mut preserved = declaration_without_comments.trim_end().to_string();
    if !preserved.ends_with(';') {
      preserved.push(';');
    }
    preserved
  } else {
    let regenerated = generate_code_from_source(&declaration_without_comments);
    let result = regenerated.trim_end().to_string();
    let result = collapse_generic_params(&result);
    let result = fix_jsdoc_star_indent(&result);
    expand_multi_declarator_var(&result)
  };

  if leading_comments.is_empty() {
    result
  } else {
    format!("{}\n{}", leading_comments.join("\n"), result)
  }
}

pub fn generate_declaration_with_source_map(
  source: &str,
  source_path: &str,
) -> (String, Option<SourceMap<'static>>) {
  let source = source.trim_end();
  if source.is_empty() {
    return (String::new(), None);
  }

  let leading_comments = extract_leading_comments(source);
  let declaration_without_comments = prepare_declaration_body(source);
  let (regenerated, map) =
    generate_code_with_source_map(&declaration_without_comments, source_path);
  let mut result = regenerated.trim_end().to_string();
  result = collapse_generic_params(&result);
  result = fix_jsdoc_star_indent(&result);

  let result = if leading_comments.is_empty() {
    result
  } else {
    format!("{}\n{}", leading_comments.join("\n"), result)
  };
  (result, map)
}

/// Expand initializer multi-declarators to Babel-style multiline
/// (`declare const a = 3, b = 3;` → broken across lines). Type-annotated forms stay one line.
fn expand_multi_declarator_var(code: &str) -> String {
  let re =
    regex::Regex::new(r"(?m)^(declare\s+(?:const|let|var)\s+)([^;\n]+,[^;\n]+);?\s*$").unwrap();
  re.replace_all(code, |caps: &regex::Captures| {
    let prefix = &caps[1];
    let body = caps[2].trim();
    if !body.contains('=') {
      return caps[0].to_string();
    }
    let parts: Vec<&str> = body.split(", ").collect();
    if parts.len() < 2 {
      return caps[0].to_string();
    }
    let mut out = String::new();
    out.push_str(prefix);
    for (i, part) in parts.iter().enumerate() {
      if i > 0 {
        out.push_str(",\n  ");
      }
      out.push_str(part.trim());
    }
    out.push(';');
    out
  })
  .to_string()
}

/// True when we must keep original multi-line var formatting (oxc_codegen collapses it).
fn should_preserve_var_decl_formatting(source: &str) -> bool {
  let trimmed = source.trim_start();
  // `declare const enum` also starts with `declare const `.
  if trimmed.contains(" enum ") || trimmed.contains("\nenum ") {
    return false;
  }
  let is_var = trimmed.starts_with("declare const ")
    || trimmed.starts_with("declare let ")
    || trimmed.starts_with("declare var ")
    || trimmed.starts_with("const ")
    || trimmed.starts_with("let ")
    || trimmed.starts_with("var ");
  is_var && (source.contains(",\n") || source.contains(",\r\n"))
}

fn prepare_declaration_body(source: &str) -> String {
  let trimmed_start = source.trim_start();
  if trimmed_start.starts_with("declare ") {
    source.to_string()
  } else if trimmed_start.starts_with("export declare ") {
    let without_export = trimmed_start.strip_prefix("export ").unwrap();
    without_export.to_string()
  } else if trimmed_start.starts_with("export default ") {
    let without_export_default = trimmed_start.strip_prefix("export default ").unwrap();
    if without_export_default.starts_with("class ")
      || without_export_default.starts_with("abstract ")
      || without_export_default.starts_with("function ")
      || without_export_default.starts_with("function<")
      || without_export_default.starts_with("function(")
    {
      format!("declare {without_export_default}")
    } else {
      without_export_default.to_string()
    }
  } else if trimmed_start.starts_with("export ") {
    let without_export = trimmed_start.strip_prefix("export ").unwrap();
    if without_export.starts_with("type ") || without_export.starts_with("interface ") {
      without_export.to_string()
    } else {
      format!("declare {without_export}")
    }
  } else if trimmed_start.starts_with("type ") || trimmed_start.starts_with("interface ") {
    source.to_string()
  } else {
    format!("declare {trimmed_start}")
  }
}

fn extract_leading_comments(source: &str) -> Vec<String> {
  let mut comments = Vec::new();
  let mut in_block_comment = false;
  let mut block_comment_lines = Vec::new();

  for line in source.lines() {
    let trimmed = line.trim();

    if in_block_comment {
      block_comment_lines.push(line.to_string());
      if trimmed.contains("*/") {
        comments.append(&mut block_comment_lines);
        in_block_comment = false;
      }
      continue;
    }

    if trimmed.starts_with("/**") || trimmed.starts_with("/*") {
      in_block_comment = true;
      block_comment_lines.push(line.to_string());
      if trimmed.contains("*/") {
        comments.append(&mut block_comment_lines);
        in_block_comment = false;
      }
      continue;
    }

    if trimmed.starts_with("//") {
      comments.push(line.to_string());
      continue;
    }

    if trimmed.is_empty() {
      if !comments.is_empty() {
        comments.push(String::new());
      }
      continue;
    }

    break;
  }

  while comments.last().is_some_and(|l| l.trim().is_empty()) {
    comments.pop();
  }

  comments
}

/// Fix oxc_codegen JSDoc formatting where continuation stars lose one space:
/// `  /**` / `  * foo` → `  /**` / `   * foo`
fn fix_jsdoc_star_indent(code: &str) -> String {
  let lines: Vec<&str> = code.lines().collect();
  let mut out: Vec<String> = Vec::with_capacity(lines.len());
  let mut i = 0;
  while i < lines.len() {
    let line = lines[i];
    let trimmed = line.trim_start();
    if trimmed.starts_with("/**") && !trimmed.contains("*/") {
      let indent = line.len() - trimmed.len();
      out.push(line.to_string());
      i += 1;
      let star_indent = " ".repeat(indent + 1);
      while i < lines.len() {
        let cont = lines[i];
        let cont_trim = cont.trim_start();
        if cont_trim.starts_with('*') {
          out.push(format!("{star_indent}{cont_trim}"));
          let done = cont_trim.contains("*/");
          i += 1;
          if done {
            break;
          }
        } else {
          break;
        }
      }
      continue;
    }
    out.push(line.to_string());
    i += 1;
  }
  let mut result = out.join("\n");
  if code.ends_with('\n') {
    result.push('\n');
  }
  result
}

fn collapse_generic_params(code: &str) -> String {
  let re = regex::Regex::new(r"<\n(\s+\w[^>]*)\n\s*>").unwrap();
  re.replace_all(code, |caps: &regex::Captures| {
    let inner = caps[1].trim();
    let collapsed = inner.split('\n').map(str::trim).collect::<Vec<_>>().join(" ");
    format!("<{collapsed}>")
  })
  .to_string()
}

#[cfg(test)]
mod declaration_codegen_tests {
  use super::*;

  #[test]
  fn test_extract_span_text() {
    let code = "hello world";
    let span = Span::new(0, 5);
    assert_eq!(extract_span_text(code, span), "hello");
    assert_eq!(extract_source_text(code, 6, 11), "world");
  }

  #[test]
  fn test_generate_declaration_with_source_map() {
    let (code, map) =
      generate_declaration_with_source_map("export interface Obj { x: number }", "foo.ts");
    assert!(code.contains("interface Obj"));
    assert!(map.is_some());
    assert!(!map.unwrap().to_json_string().contains("\"mappings\":\"\""));
  }
}
