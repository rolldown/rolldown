use std::{path::Path, sync::Arc};

use oxc::span::SourceType as OxcSourceType;
use rolldown_common::{ModuleType, NormalizedBundlerOptions};
use rolldown_loader_utils::{base64_to_esm, binary_to_esm, json_to_esm, text_to_esm};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

use super::transformer::transform;

use crate::runtime::ROLLDOWN_RUNTIME_RESOURCE_ID;

fn pure_esm_js_oxc_source_type() -> OxcSourceType {
  let pure_esm_js = OxcSourceType::default().with_module(true);
  debug_assert!(pure_esm_js.is_javascript());
  debug_assert!(!pure_esm_js.is_jsx());
  debug_assert!(pure_esm_js.is_module());
  debug_assert!(pure_esm_js.is_strict());

  pure_esm_js
}

#[allow(dead_code)]
enum ParseType {
  Js,
  Jsx,
  Ts,
  Tsx,
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
    ModuleType::Js => (source, ParseType::Js),
    ModuleType::Jsx => (source, ParseType::Jsx),
    ModuleType::Ts => (source, ParseType::Ts),
    ModuleType::Tsx => (source, ParseType::Tsx),
    ModuleType::Json => (json_to_esm(&source)?.into(), ParseType::Js),
    ModuleType::Text => (text_to_esm(&source)?.into(), ParseType::Js),
    ModuleType::Base64 => (base64_to_esm(&source).into(), ParseType::Js),
    ModuleType::Binary => {
      (binary_to_esm(&source, options.platform, ROLLDOWN_RUNTIME_RESOURCE_ID).into(), ParseType::Js)
    }
  };

  let source_type = pure_esm_js_oxc_source_type();
  // 2. Parse the source to AST and transform non-js AST to valid JS AST.
  let valid_js_ast = match parsed_type {
    ParseType::Js => OxcCompiler::parse(Arc::clone(&source), source_type)?,
    ParseType::Jsx => transform(path, Arc::clone(&source), source_type.with_jsx(true))?,
    ParseType::Ts => transform(path, Arc::clone(&source), source_type.with_typescript(true))?,
    ParseType::Tsx => {
      transform(path, Arc::clone(&source), source_type.with_typescript(true).with_jsx(true))?
    }
  };

  Ok(valid_js_ast)
}
