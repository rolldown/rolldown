use oxc::allocator::Allocator;
use oxc::ast_visit::Visit;
use oxc::span::GetSpan;
use std::sync::{Arc, Mutex};

use crate::{
  codegen,
  dependencies::DependencyCollector,
  filename,
  helpers::HelperTransformer,
  import_export::ImportExportRewriter,
  parser::TypeScriptParser,
  type_params::TypeParamCollector,
  types::{ChunkInfo, DeclarationInfo, FakeJsOptions, PluginState, Result, TransformResult},
  visitor::DeclarationCollector,
};

#[derive(Debug)]
pub struct FakeJsPlugin {
  options: FakeJsOptions,
  state: Arc<Mutex<PluginState>>,
}

impl FakeJsPlugin {
  pub fn new(options: FakeJsOptions) -> Self {
    Self { options, state: Arc::new(Mutex::new(PluginState::new())) }
  }

  pub fn transform(&self, code: &str, id: &str) -> Result<TransformResult> {
    if !filename::is_dts(id) {
      return Ok(TransformResult { code: code.to_string(), map: None });
    }

    let mut state = self.state.lock().unwrap();
    let transformed = self.transform_declarations(code, id, &mut state)?;

    Ok(TransformResult {
      code: transformed,
      map: if self.options.sourcemap { Some(Self::generate_sourcemap(id)) } else { None },
    })
  }

  fn transform_declarations(
    &self,
    code: &str,
    id: &str,
    state: &mut PluginState,
  ) -> Result<String> {
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);

    let parse_result = parser.parse(code, id)?;

    let directives =
      crate::ast_utils::collect_reference_directives_from_program(&parse_result.program, code);
    if !directives.is_empty() {
      state.comments_map.insert(id.to_string(), directives);
    }

    let mut collector = DeclarationCollector::new();
    collector.visit_program(&parse_result.program);

    let mut output = Vec::new();
    let mut type_only_ids = Vec::new();

    for stmt in &parse_result.program.body {
      if let Some(rewritten) =
        ImportExportRewriter::rewrite_statement(stmt, code, &mut type_only_ids)
      {
        if !matches!(
          stmt,
          oxc::ast::ast::Statement::TSInterfaceDeclaration(_)
            | oxc::ast::ast::Statement::TSTypeAliasDeclaration(_)
            | oxc::ast::ast::Statement::TSEnumDeclaration(_)
            | oxc::ast::ast::Statement::FunctionDeclaration(_)
            | oxc::ast::ast::Statement::ClassDeclaration(_)
            | oxc::ast::ast::Statement::VariableDeclaration(_)
            | oxc::ast::ast::Statement::TSModuleDeclaration(_)
        ) {
          output.push(rewritten);
        }
      }
    }

    for decl_node in collector.declarations {
      let transformed =
        Self::transform_declaration_node(&decl_node, code, &parse_result.program, state);
      output.push(transformed);
    }

    if self.options.side_effects {
      output.push("sideEffect();".to_string());
    }

    state.type_only_map.insert(id.to_string(), type_only_ids);

    Ok(output.join("\n"))
  }

  fn transform_declaration_node(
    decl_node: &crate::visitor::DeclarationNode,
    source: &str,
    program: &oxc::ast::ast::Program,
    state: &mut PluginState,
  ) -> String {
    let bindings = &decl_node.bindings;

    if bindings.is_empty() {
      return String::new();
    }

    let decl_source =
      codegen::extract_source_text(source, decl_node.span.start, decl_node.span.end);

    let mut type_param_collector = TypeParamCollector::new();
    let mut dep_collector = DependencyCollector::new(bindings.clone());

    type_param_collector.visit_program(program);
    dep_collector.visit_program(program);

    let type_params = type_param_collector.into_params();
    let deps: Vec<String> = dep_collector.deps.into_iter().collect();

    let decl_info = DeclarationInfo {
      id: 0,
      bindings: bindings.clone(),
      type_params,
      deps: deps.clone(),
      children: vec![],
      source: decl_source,
      is_side_effect: decl_node.is_side_effect,
    };

    let decl_id = state.register_declaration(decl_info);

    let type_param_names: Vec<String> =
      state.get_declaration(decl_id).unwrap().type_params.iter().map(|p| p.name.clone()).collect();

    let runtime_binding = codegen::RuntimeBindingGenerator::generate_runtime_binding(
      &bindings[0],
      decl_id,
      &deps,
      &type_param_names,
      decl_node.is_side_effect,
    );

    if decl_node.is_export {
      if decl_node.is_default {
        let export_line = format!("export {{ {} as default }}", bindings[0]);
        format!("{runtime_binding}\n{export_line}")
      } else {
        format!("export {runtime_binding}")
      }
    } else {
      runtime_binding
    }
  }

  pub fn render_chunk(&self, code: &str, chunk: &ChunkInfo) -> Result<String> {
    if !filename::is_dts(&chunk.filename) {
      return Ok(code.to_string());
    }

    let state = self.state.lock().unwrap();

    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let parse_result = parser.parse(code, &chunk.filename)?;

    let transformed_stmts =
      HelperTransformer::transform_statements(&parse_result.program.body, code);

    let mut output = Vec::new();

    for stmt in &parse_result.program.body {
      if let Some(transformed) = Self::process_statement(stmt, code, &state) {
        output.push(transformed);
      }
    }

    let mut final_output = Vec::new();

    for transformed in transformed_stmts {
      if !transformed.trim().is_empty() {
        final_output.push(transformed);
      }
    }

    for stmt_output in output {
      if !stmt_output.trim().is_empty() {
        final_output.push(stmt_output);
      }
    }

    let mut comments = Vec::new();
    for module_id in &chunk.module_ids {
      if let Some(module_comments) = state.comments_map.get(module_id) {
        comments.extend(module_comments.clone());
      }
    }

    let mut result = String::new();
    if !comments.is_empty() {
      for comment in comments {
        result.push_str(&comment);
        result.push('\n');
      }
    }

    result.push_str(&final_output.join("\n"));

    if result.trim().is_empty() {
      return Ok("export { };".to_string());
    }

    Ok(result)
  }

  fn process_statement(
    stmt: &oxc::ast::ast::Statement,
    source: &str,
    state: &PluginState,
  ) -> Option<String> {
    use oxc::ast::ast::Statement;

    match stmt {
      Statement::VariableDeclaration(var_decl) => {
        if let Some(decl_id) = Self::extract_declaration_id(var_decl) {
          if let Some(decl_info) = state.get_declaration(decl_id) {
            return Some(format!("declare {}", decl_info.source));
          }
        }
        None
      }
      Statement::ExportNamedDeclaration(export) => {
        if let Some(oxc::ast::ast::Declaration::VariableDeclaration(var_decl)) = &export.declaration
        {
          if let Some(decl_id) = Self::extract_declaration_id(var_decl) {
            if let Some(decl_info) = state.get_declaration(decl_id) {
              return Some(format!("export declare {}", decl_info.source));
            }
          }
        }
        Some(codegen::extract_source_text(source, stmt.span().start, stmt.span().end))
      }
      Statement::ExpressionStatement(_) => None,
      Statement::ImportDeclaration(_) => {
        let import_text = codegen::extract_source_text(source, stmt.span().start, stmt.span().end);
        Some(Self::patch_import_source(&import_text))
      }
      _ => Some(codegen::extract_source_text(source, stmt.span().start, stmt.span().end)),
    }
  }

  fn extract_declaration_id(var_decl: &oxc::ast::ast::VariableDeclaration) -> Option<usize> {
    if var_decl.declarations.len() != 1 {
      return None;
    }

    let declarator = &var_decl.declarations[0];
    let init = declarator.init.as_ref()?;

    if let oxc::ast::ast::Expression::ArrayExpression(arr) = init {
      if arr.elements.is_empty() {
        return None;
      }

      if let Some(oxc::ast::ast::ArrayExpressionElement::NumericLiteral(num)) = arr.elements.first()
      {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        return Some(num.value as usize);
      }
    }

    None
  }

  fn patch_import_source(import_text: &str) -> String {
    let re_double = regex::Regex::new(r#""([^"]+)\.d\.(ts|mts|cts)""#).unwrap();
    let re_single = regex::Regex::new(r"'([^']+)\.d\.(ts|mts|cts)'").unwrap();

    let result = re_double.replace_all(import_text, |caps: &regex::Captures| {
      let path = &caps[1];
      let ext = &caps[2];
      let js_ext = match ext {
        "mts" => "mjs",
        "cts" => "cjs",
        _ => "js",
      };
      format!("\"{path}.{js_ext}\"")
    });

    let result = re_single.replace_all(&result, |caps: &regex::Captures| {
      let path = &caps[1];
      let ext = &caps[2];
      let js_ext = match ext {
        "mts" => "mjs",
        "cts" => "cjs",
        _ => "js",
      };
      format!("'{path}.{js_ext}'")
    });

    result.to_string()
  }

  fn generate_sourcemap(filename: &str) -> String {
    format!(r#"{{"version":3,"file":"{filename}","sources":["{filename}"],"mappings":""}}"#)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_plugin_creation() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    assert!(!plugin.options.sourcemap);
  }

  #[test]
  fn test_transform_non_dts() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code = "const x = 1;";
    let result = plugin.transform(code, "test.ts").unwrap();
    assert_eq!(result.code, code);
  }

  #[test]
  fn test_transform_simple_interface() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code = "export interface Foo { bar: string; }";
    let result = plugin.transform(code, "test.d.ts").unwrap();
    assert!(result.code.contains("var Foo"));
    assert!(result.code.contains("export"));
  }
}
