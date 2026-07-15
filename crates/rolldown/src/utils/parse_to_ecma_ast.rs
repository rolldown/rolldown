use std::{borrow::Cow, path::Path};

use json_escape_simd::escape;
use oxc::{ast::ast::Statement, semantic::Scoping, span::SourceType as OxcSourceType};
use oxc_str::CompactStr;
use rolldown_common::{
  ConstExportMeta, ModuleDefFormat, ModuleId, ModuleType, NormalizedBundlerOptions,
  RUNTIME_MODULE_KEY, StrOrBytes, json_value_to_ecma_ast,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, BuildResult, EventKind, EventKindSwitcher};
use rolldown_plugin::HookTransformAstArgs;
use rolldown_utils::mime::guess_mime;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath as _;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::{module_factory::CreateModuleContext, oxc_parse_type::OxcParseType};

#[inline]
fn pure_esm_js_oxc_source_type(module_def_format: ModuleDefFormat) -> OxcSourceType {
  let default_source_type = OxcSourceType::default();
  debug_assert!(default_source_type.is_javascript());
  debug_assert!(!default_source_type.is_jsx());
  match module_def_format {
    ModuleDefFormat::Cjs | ModuleDefFormat::Cts => default_source_type.with_commonjs(true),
    ModuleDefFormat::EsmMjs | ModuleDefFormat::EsmMts | ModuleDefFormat::EsmPackageJson => {
      default_source_type.with_module(true)
    }
    ModuleDefFormat::CjsPackageJson | ModuleDefFormat::Unknown => {
      // treat unknown format as ESM for now: https://github.com/rolldown/rolldown/issues/7009
      default_source_type.with_module(true)
    }
  }
}

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub has_lazy_export: bool,
  /// Body index of the loader-created lazy-export payload after `transformAst` and preprocessing.
  /// The scanner turns this transient identity into a statement metadata bit consumed by link.
  pub lazy_export_payload_stmt_index: Option<usize>,
  pub warnings: Vec<BuildDiagnostic>,
  /// Whether JSX syntax should be preserved in the output, determined per-module
  /// during transformation based on the resolved tsconfig.
  pub preserve_jsx: bool,
  /// Enum member constant values, keyed by enum name → member name → value.
  /// Used by the finalizer to inline cross-module enum member accesses (e.g., `Direction.Up` → `0`).
  pub enum_member_value_map: FxHashMap<CompactStr, FxHashMap<CompactStr, ConstExportMeta>>,
}

pub async fn parse_to_ecma_ast(
  ctx: &CreateModuleContext<'_>,
  source: StrOrBytes,
) -> BuildResult<ParseToEcmaAstResult> {
  let CreateModuleContext {
    options,
    stable_id,
    resolved_id,
    module_type,
    plugin_driver,
    replace_global_define_config,
    ..
  } = ctx;

  let path = Path::new(resolved_id.id.as_str());
  let is_user_defined_entry = ctx.is_user_defined_entry;

  let (has_lazy_export, source, parsed_type) =
    pre_process_source(path, source, module_type, options)?;

  let oxc_source_type = {
    let default = pure_esm_js_oxc_source_type(resolved_id.module_def_format);
    match parsed_type {
      OxcParseType::Js => default,
      OxcParseType::Jsx => default.with_jsx(!options.transform_options.is_jsx_disabled()),
      OxcParseType::Ts => default.with_typescript(true),
      OxcParseType::Tsx => {
        default.with_typescript(true).with_jsx(!options.transform_options.is_jsx_disabled())
      }
    }
  };

  let mut ecma_ast = match module_type {
    ModuleType::Json => {
      let json_value: serde_json::Value = serde_json::from_str(&source).map_err(|e| {
        let line = e.line() - 1;
        // Convert to 0-indexed column. serde_json returns 1-indexed columns (though possibly 0 in some edge cases).
        // See: https://docs.rs/serde_json/1.0.132/serde_json/struct.Error.html#method.column
        let column = e.column().saturating_sub(1);
        BuildDiagnostic::json_parse(
          resolved_id.id.as_str().into(),
          source.as_ref().into(),
          line,
          column,
          e.to_string().into(),
        )
      })?;
      json_value_to_ecma_ast(&json_value)
    }
    ModuleType::Dataurl | ModuleType::Base64 | ModuleType::Text => {
      EcmaCompiler::parse_expr_as_program(resolved_id.id.as_str(), source, oxc_source_type)?
    }
    _ => EcmaCompiler::parse(resolved_id.id.as_str(), source, oxc_source_type)?,
  };
  let lazy_export_payload_identity =
    has_lazy_export.then(|| capture_lazy_export_payload_identity(&ecma_ast)).flatten();

  ecma_ast = plugin_driver
    .transform_ast(HookTransformAstArgs {
      cwd: &options.cwd,
      ast: ecma_ast,
      id: resolved_id.id.as_str(),
      stable_id,
      is_user_defined_entry,
      module_type,
    })
    .await?;

  let is_local_project_file = is_local_project_file(&resolved_id.id, &options.normalized_cwd);
  let should_warn_on_invalid_annotation =
    options.checks.contains(EventKindSwitcher::InvalidAnnotation) && is_local_project_file;

  let mut result = PreProcessEcmaAst::default().build(
    ecma_ast,
    stable_id,
    resolved_id.id.as_str(),
    should_warn_on_invalid_annotation,
    &parsed_type,
    replace_global_define_config.as_ref(),
    options,
    has_lazy_export,
  )?;
  if has_lazy_export {
    result.lazy_export_payload_stmt_index = Some(
      match resolve_lazy_export_payload(&result.ast, lazy_export_payload_identity) {
        Ok(body_index) => body_index,
        Err(reason) => {
          return Err(BuildDiagnostic::oxc_error(
            result.ast.source().clone(),
            resolved_id.id.as_str().to_string(),
            "A transformAst hook may move, edit, or clone the loader-created payload statement, but it must leave that statement uniquely identifiable.".to_string(),
            format!(
              "Could not identify the loader-created lazy-export payload for `{}`: {reason}",
              resolved_id.id.as_str()
            ),
            Vec::new(),
            EventKind::TransformError,
          ))?;
        }
      },
    );
  }
  Ok(result)
}

#[derive(Clone, Copy)]
struct LazyExportPayloadIdentity {
  original_identity: usize,
}

/// Captures a side-channel identity without changing the AST exposed to `transformAst` hooks.
///
/// Moving the allocator-backed statement preserves this address. A hook that replaces the whole
/// statement loses the identity; Parse then accepts the result only when there is exactly one
/// expression-statement candidate, and otherwise fails before Scan instead of guessing.
fn capture_lazy_export_payload_identity(ecma_ast: &EcmaAst) -> Option<LazyExportPayloadIdentity> {
  let statement = ecma_ast.program().body.first()?;
  let Statement::ExpressionStatement(statement) = statement else { return None };
  Some(LazyExportPayloadIdentity { original_identity: std::ptr::from_ref(&**statement) as usize })
}

fn resolve_lazy_export_payload(
  ecma_ast: &EcmaAst,
  identity: Option<LazyExportPayloadIdentity>,
) -> Result<usize, &'static str> {
  let mut identity_match = None;
  let mut expression_match = None;
  let mut expression_match_count = 0usize;

  for (body_index, statement) in ecma_ast.program().body.iter().enumerate() {
    if let Some(statement_identity) = expression_statement_identity(statement) {
      expression_match = Some(body_index);
      expression_match_count += 1;
      if identity.is_some_and(|payload| payload.original_identity == statement_identity) {
        identity_match = Some(body_index);
      }
    }
  }

  let body_index = if let Some(body_index) = identity_match {
    body_index
  } else if expression_match_count == 1 {
    let Some(body_index) = expression_match else { return Err("missing payload expression") };
    body_index
  } else if expression_match_count == 0 {
    return Err("the transformed module contains no payload expression");
  } else {
    return Err(
      "the transformed module contains multiple payload candidates after identity was lost",
    );
  };

  let Some(statement) = ecma_ast.program().body.get(body_index) else {
    return Err("the selected payload is outside the transformed module");
  };
  if !std::matches!(statement, Statement::ExpressionStatement(_)) {
    return Err("the selected payload is not an expression statement");
  }
  Ok(body_index)
}

fn expression_statement_identity(statement: &Statement<'_>) -> Option<usize> {
  let Statement::ExpressionStatement(statement) = statement else { return None };
  Some(std::ptr::from_ref(&**statement) as usize)
}

fn is_local_project_file(id: &ModuleId, normalized_cwd: &Path) -> bool {
  if id.is_in_node_modules() {
    return false;
  }

  id.as_path().is_some_and(|path| path.normalize().starts_with(normalized_cwd))
}

fn pre_process_source(
  path: &Path,
  source: StrOrBytes,
  module_type: &ModuleType,
  options: &NormalizedBundlerOptions,
) -> BuildResult<(bool, Cow<'static, str>, OxcParseType)> {
  let has_lazy_export = matches!(
    module_type,
    ModuleType::Json | ModuleType::Text | ModuleType::Base64 | ModuleType::Dataurl
  );

  let source = match module_type {
    ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx | ModuleType::Json => {
      Cow::Owned(source.try_into_string()?)
    }
    ModuleType::Css => {
      unreachable!("CSS modules should error before reaching parse_to_ecma_ast")
    }
    ModuleType::Text => {
      let text = source.try_into_string()?;
      // Strip UTF-8 BOM if present
      let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
      Cow::Owned(escape(text))
    }
    ModuleType::Asset => {
      return Err(anyhow::format_err!(
        "Encountered a module with type `asset` during AST parsing. \
         Modules with type `asset` must be handled by the builtin AssetModulePlugin before this stage; \
         please check your plugin and loader configuration."
      ))?;
    }
    ModuleType::Base64 => {
      let encoded = rolldown_utils::base64::to_standard_base64(source.as_bytes());
      Cow::Owned(escape(&encoded))
    }
    ModuleType::Dataurl => {
      let data = source.as_bytes();
      let guessed_mime = guess_mime(path, data)?;
      let dataurl = rolldown_utils::dataurl::encode_as_shortest_dataurl(&guessed_mime, data);
      Cow::Owned(escape(&dataurl))
    }
    ModuleType::Binary => {
      let encoded = rolldown_utils::base64::to_standard_base64(source.as_bytes());
      let to_binary = match options.platform {
        rolldown_common::Platform::Node => "__toBinaryNode",
        _ => "__toBinary",
      };
      Cow::Owned(rolldown_utils::concat_string!(
        "import {",
        to_binary,
        "} from '",
        RUNTIME_MODULE_KEY,
        "'; export default ",
        to_binary,
        "('",
        encoded,
        "')"
      ))
    }
    ModuleType::Empty => Cow::Borrowed(""),
    ModuleType::Copy => {
      return Err(anyhow::format_err!(
        "Encountered a module with type `copy` during AST parsing. \
         Modules with type `copy` must be handled by the builtin CopyModulePlugin before this stage; \
         please check your plugin and loader configuration."
      ))?;
    }
    ModuleType::Custom(custom_type) => {
      // TODO: should provide friendly error message to say that this type is not supported by rolldown.
      // Users should handle this type in load/transform hooks
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };

  Ok((has_lazy_export, source, module_type.into()))
}

#[cfg(test)]
mod tests {
  use oxc::{
    allocator::CloneIn,
    ast::ast::{Expression, ParenthesizedExpression, Statement},
    span::SPAN,
  };
  use rolldown_common::{ModuleId, json_value_to_ecma_ast};
  use rolldown_ecmascript_utils::AstFactory;
  use sugar_path::SugarPath as _;

  use super::{
    capture_lazy_export_payload_identity, is_local_project_file, resolve_lazy_export_payload,
  };

  fn append_four_parenthesized_expression(ast: &mut rolldown_ecmascript::EcmaAst) {
    ast.program.with_mut(|fields| {
      let ast_factory = AstFactory::new(fields.allocator);
      let mut expression = ast_factory.make_id_ref_expr(SPAN, "sideEffect");
      for _ in 0..4 {
        expression = Expression::ParenthesizedExpression(ParenthesizedExpression::boxed(
          SPAN,
          expression,
          &ast_factory,
        ));
      }
      fields.program.body.push(Statement::new_expression_statement(SPAN, expression, &ast_factory));
    });
  }

  #[test]
  fn payload_identity_survives_a_statement_move() {
    let mut ast = json_value_to_ecma_ast(&serde_json::json!({ "value": 1 }));
    let identity = capture_lazy_export_payload_identity(&ast).expect("payload identity");

    ast.program.with_mut(|fields| {
      let payload = fields.program.body.remove(0);
      let ast_factory = AstFactory::new(fields.allocator);
      fields.program.body.push(Statement::new_expression_statement(
        SPAN,
        ast_factory.make_id_ref_expr(SPAN, "sideEffect"),
        &ast_factory,
      ));
      fields.program.body.push(payload);
    });

    assert_eq!(resolve_lazy_export_payload(&ast, Some(identity)), Ok(1));
  }

  #[test]
  fn unique_whole_statement_clone_uses_the_compatibility_fallback() {
    let mut ast = json_value_to_ecma_ast(&serde_json::json!({ "value": 1 }));
    let identity = capture_lazy_export_payload_identity(&ast).expect("payload identity");

    ast.program.with_mut(|fields| {
      fields.program.body[0] = fields.program.body[0].clone_in(fields.allocator);
    });

    assert_eq!(resolve_lazy_export_payload(&ast, Some(identity)), Ok(0));
  }

  #[test]
  fn unrelated_parentheses_do_not_conflict_with_retained_identity() {
    let mut ast = json_value_to_ecma_ast(&serde_json::json!({ "value": 1 }));
    let identity = capture_lazy_export_payload_identity(&ast).expect("payload identity");

    append_four_parenthesized_expression(&mut ast);

    assert_eq!(resolve_lazy_export_payload(&ast, Some(identity)), Ok(0));
    let Statement::ExpressionStatement(unrelated) = &ast.program().body[1] else {
      panic!("unrelated expression")
    };
    assert!(matches!(unrelated.expression, Expression::ParenthesizedExpression(_)));
  }

  #[test]
  fn unrelated_parentheses_cannot_impersonate_a_replaced_payload() {
    let mut ast = json_value_to_ecma_ast(&serde_json::json!({ "value": 1 }));
    let identity = capture_lazy_export_payload_identity(&ast).expect("payload identity");

    ast.program.with_mut(|fields| {
      fields.program.body[0] = fields.program.body[0].clone_in(fields.allocator);
    });
    append_four_parenthesized_expression(&mut ast);

    assert_eq!(
      resolve_lazy_export_payload(&ast, Some(identity)),
      Err("the transformed module contains multiple payload candidates after identity was lost")
    );
  }

  #[test]
  fn parent_dir_cannot_escape_cwd() {
    let cwd = std::env::current_dir().unwrap().join("project");
    let normalized_cwd = cwd.normalize();
    let id = ModuleId::new(cwd.join("src/../../dependency.js").to_string_lossy().into_owned());

    assert!(!is_local_project_file(&id, normalized_cwd.as_ref()));
  }

  #[test]
  fn node_modules_component_is_excluded() {
    let cwd = std::env::current_dir().unwrap().join("project");
    let normalized_cwd = cwd.normalize();
    let id = ModuleId::new(cwd.join("node_modules/pkg/index.js").to_string_lossy().into_owned());

    assert!(!is_local_project_file(&id, normalized_cwd.as_ref()));
  }
}
