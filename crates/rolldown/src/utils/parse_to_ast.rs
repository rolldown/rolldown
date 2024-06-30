use std::{path::Path, sync::Arc};

use oxc::span::SourceType as OxcSourceType;
use rolldown_common::{ModuleType, NormalizedBundlerOptions};
use rolldown_loader_utils::{binary_to_esm, json_to_esm, raw_text_to_esm, text_to_esm};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

use super::pre_process_ast::pre_process_ast;

use crate::{runtime::ROLLDOWN_RUNTIME_RESOURCE_ID, types::oxc_parse_type::OxcParseType};

fn pure_esm_js_oxc_source_type() -> OxcSourceType {
  let pure_esm_js = OxcSourceType::default().with_module(true);
  debug_assert!(pure_esm_js.is_javascript());
  debug_assert!(!pure_esm_js.is_jsx());
  debug_assert!(pure_esm_js.is_module());
  debug_assert!(pure_esm_js.is_strict());

  pure_esm_js
}

pub fn parse_to_ast(
  path: &Path,
  options: &NormalizedBundlerOptions,
  module_type: ModuleType,
  source: impl Into<Arc<str>>,
) -> anyhow::Result<OxcAst> {
  let source: Arc<str> = source.into();

  // 1. Transform the source to the type that rolldown supported.
  let (source, parsed_type) = match module_type {
    ModuleType::Js => (source, OxcParseType::Js),
    ModuleType::Jsx => (source, OxcParseType::Jsx),
    ModuleType::Ts => (source, OxcParseType::Ts),
    ModuleType::Tsx => (source, OxcParseType::Tsx),
    ModuleType::Json => (json_to_esm(&source)?.into(), OxcParseType::Js),
    ModuleType::Text => (text_to_esm(&source)?.into(), OxcParseType::Js),
    ModuleType::Base64 | ModuleType::DataUrl => (base64_to_esm(&source).into(), OxcParseType::Js),
    ModuleType::Binary => (
      binary_to_esm(&source, options.platform, ROLLDOWN_RUNTIME_RESOURCE_ID).into(),
      OxcParseType::Js,
    ),
    ModuleType::Empty => ("export {}".to_string().into(), OxcParseType::Js),
  };

  let oxc_source_type = {
    let default = pure_esm_js_oxc_source_type();
    match parsed_type {
      OxcParseType::Js => default,
      OxcParseType::Jsx => default.with_jsx(true),
      OxcParseType::Ts => default.with_typescript(true),
      OxcParseType::Tsx => default.with_typescript(true).with_jsx(true),
    }
  };

  let oxc_ast = OxcCompiler::parse(Arc::clone(&source), oxc_source_type)?;

  let valid_js_ast = pre_process_ast(oxc_ast, &parsed_type, path, oxc_source_type)?;

  Ok(valid_js_ast)
}
