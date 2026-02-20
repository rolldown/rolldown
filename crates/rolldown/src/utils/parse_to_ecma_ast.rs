use std::{borrow::Cow, path::Path};

use json_escape_simd::escape;
use oxc::{semantic::Scoping, span::SourceType as OxcSourceType};
use rolldown_common::{
  ModuleType, NormalizedBundlerOptions, RUNTIME_MODULE_KEY, StrOrBytes, json_value_to_ecma_ast,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::HookTransformAstArgs;
use rolldown_utils::mime::guess_mime;
use sugar_path::SugarPath;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::{module_factory::CreateModuleContext, oxc_parse_type::OxcParseType};

#[inline]
fn pure_esm_js_oxc_source_type() -> OxcSourceType {
  let pure_esm_js = OxcSourceType::default().with_module(true);
  debug_assert!(pure_esm_js.is_javascript());
  debug_assert!(!pure_esm_js.is_jsx());
  debug_assert!(pure_esm_js.is_module());
  debug_assert!(pure_esm_js.is_strict());

  pure_esm_js
}

pub struct ParseToEcmaAstResult {
  pub ast: EcmaAst,
  pub scoping: Scoping,
  pub has_lazy_export: bool,
  pub warnings: Vec<BuildDiagnostic>,
  /// Whether JSX syntax should be preserved in the output, determined per-module
  /// during transformation based on the resolved tsconfig.
  pub preserve_jsx: bool,
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

  let path = resolved_id.id.as_path();
  let is_user_defined_entry = ctx.is_user_defined_entry;

  let (has_lazy_export, source, parsed_type) =
    pre_process_source(path, source, module_type, is_user_defined_entry, options)?;

  let oxc_source_type = {
    let default = pure_esm_js_oxc_source_type();
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

  PreProcessEcmaAst::default().build(
    ecma_ast,
    stable_id,
    resolved_id.id.as_str(),
    &parsed_type,
    replace_global_define_config.as_ref(),
    options,
    has_lazy_export,
  )
}

fn pre_process_source(
  path: &Path,
  source: StrOrBytes,
  module_type: &ModuleType,
  is_user_defined_entry: bool,
  options: &NormalizedBundlerOptions,
) -> BuildResult<(bool, Cow<'static, str>, OxcParseType)> {
  let mut has_lazy_export = matches!(
    module_type,
    ModuleType::Json
      | ModuleType::Text
      | ModuleType::Base64
      | ModuleType::Dataurl
      | ModuleType::Asset
  );

  let source = match module_type {
    ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx | ModuleType::Json => {
      Cow::Owned(source.try_into_string()?)
    }
    ModuleType::Css => {
      if is_user_defined_entry {
        Cow::Borrowed("export {}")
      } else {
        has_lazy_export = true;
        Cow::Borrowed("({})")
      }
    }
    ModuleType::Text => {
      let text = source.try_into_string()?;
      // Strip UTF-8 BOM if present
      let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
      Cow::Owned(escape(text))
    }
    ModuleType::Asset => Cow::Borrowed("__ROLLDOWN_ASSET_FILENAME__"),
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
    ModuleType::Custom(custom_type) => {
      // TODO: should provide friendly error message to say that this type is not supported by rolldown.
      // Users should handle this type in load/transform hooks
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };

  Ok((has_lazy_export, source, module_type.into()))
}
