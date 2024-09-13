use std::path::Path;

use arcstr::ArcStr;
use oxc::{
  minifier::ReplaceGlobalDefinesConfig,
  semantic::{ScopeTree, SymbolTable},
  span::SourceType as OxcSourceType,
};
use rolldown_common::{ModuleType, NormalizedBundlerOptions, StrOrBytes};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::DiagnosableResult;
use rolldown_loader_utils::{binary_to_esm, json_to_esm, text_to_esm};
use rolldown_plugin::{HookTransformAstArgs, PluginDriver};
use rolldown_utils::mime::guess_mime;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::{runtime::RUNTIME_MODULE_ID, types::oxc_parse_type::OxcParseType};

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
  pub symbol_table: SymbolTable,
  pub scope_tree: ScopeTree,
  pub source: ArcStr,
}

pub fn parse_to_ecma_ast(
  plugin_driver: &PluginDriver,
  path: &Path,
  stable_id: &str,
  options: &NormalizedBundlerOptions,
  module_type: &ModuleType,
  source: StrOrBytes,
  replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
) -> anyhow::Result<DiagnosableResult<ParseToEcmaAstResult>> {
  // 1. Transform the source to the type that rolldown supported.
  let (source, parsed_type) = match module_type {
    ModuleType::Js => (source.try_into_string()?, OxcParseType::Js),
    ModuleType::Jsx => (source.try_into_string()?, OxcParseType::Jsx),
    ModuleType::Ts => (source.try_into_string()?, OxcParseType::Ts),
    ModuleType::Tsx => (source.try_into_string()?, OxcParseType::Tsx),
    ModuleType::Json => {
      let content = json_to_esm(&source.try_into_string()?)?;
      (content, OxcParseType::Js)
    }
    ModuleType::Text => {
      let content = text_to_esm(&source.try_into_string()?)?;
      (content, OxcParseType::Js)
    }
    ModuleType::Base64 => {
      let source = source.try_into_bytes()?;
      let encoded = rolldown_utils::base64::to_standard_base64(source);
      (text_to_esm(&encoded)?, OxcParseType::Js)
    }
    ModuleType::Dataurl => {
      let data = source.try_into_bytes()?;
      let guessed_mime = guess_mime(path, &data)?;
      let dataurl = rolldown_utils::dataurl::encode_as_shortest_dataurl(&guessed_mime, &data);
      (text_to_esm(&dataurl)?, OxcParseType::Js)
    }
    ModuleType::Binary => {
      let source = source.try_into_bytes()?;
      let encoded = rolldown_utils::base64::to_standard_base64(source);
      (binary_to_esm(&encoded, options.platform, RUNTIME_MODULE_ID), OxcParseType::Js)
    }
    ModuleType::Empty => ("export {}".to_string(), OxcParseType::Js),
    ModuleType::Custom(custom_type) => {
      // TODO: should provide friendly error message to say that this type is not supported by rolldown.
      // Users should handle this type in load/transform hooks
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"));
    }
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

  let source = ArcStr::from(source);
  let parse_result = EcmaCompiler::parse(stable_id, &source, oxc_source_type);

  let mut ecma_ast = match parse_result {
    Ok(ecma_ast) => ecma_ast,
    Err(errs) => {
      return Ok(Err(errs));
    }
  };

  ecma_ast = plugin_driver.transform_ast(HookTransformAstArgs {
    cwd: &options.cwd,
    ast: ecma_ast,
    id: stable_id,
  })?;

  PreProcessEcmaAst::default()
    .build(ecma_ast, &parsed_type, path, oxc_source_type, replace_global_define_config, options)
    .map(|(ast, symbol_table, scope_tree)| {
      Ok(ParseToEcmaAstResult { ast, symbol_table, scope_tree, source })
    })
}
