use std::path::Path;

use arcstr::ArcStr;
use oxc::{
  semantic::{ScopeTree, SymbolTable},
  span::SourceType as OxcSourceType,
};
use rolldown_common::{ModuleType, NormalizedBundlerOptions, StrOrBytes, RUNTIME_MODULE_ID};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_loader_utils::{binary_to_esm, text_to_string_literal};
use rolldown_plugin::HookTransformAstArgs;
use rolldown_utils::mime::guess_mime;
use sugar_path::SugarPath;

use super::pre_process_ecma_ast::PreProcessEcmaAst;

use crate::types::{module_factory::CreateModuleContext, oxc_parse_type::OxcParseType};

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
  pub has_lazy_export: bool,
  pub warning: Vec<BuildDiagnostic>,
}

pub fn parse_to_ecma_ast(
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
      OxcParseType::Jsx => {
        if options.jsx.is_jsx_disabled() {
          default
        } else {
          default.with_jsx(true)
        }
      }
      OxcParseType::Ts => default.with_typescript(true),
      OxcParseType::Tsx => {
        if options.jsx.is_jsx_disabled() {
          default.with_typescript(true)
        } else {
          default.with_typescript(true).with_jsx(true)
        }
      }
    }
  };

  let mut ecma_ast = match module_type {
    ModuleType::Json | ModuleType::Dataurl | ModuleType::Base64 | ModuleType::Text => {
      EcmaCompiler::parse_expr_as_program(stable_id, source, oxc_source_type)?
    }
    _ => EcmaCompiler::parse(stable_id, source, oxc_source_type)?,
  };

  ecma_ast = plugin_driver.transform_ast(HookTransformAstArgs {
    cwd: &options.cwd,
    ast: ecma_ast,
    id: stable_id,
    is_user_defined_entry,
  })?;

  PreProcessEcmaAst::default().build(
    ecma_ast,
    stable_id,
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
) -> BuildResult<(bool, ArcStr, OxcParseType)> {
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
      source.try_into_string()?
    }
    ModuleType::Css => {
      if is_user_defined_entry {
        "export {}".to_owned()
      } else {
        has_lazy_export = true;
        "({})".to_owned()
      }
    }
    ModuleType::Text => text_to_string_literal(&source.try_into_string()?)?,
    ModuleType::Asset => "import.meta.__ROLLDOWN_ASSET_FILENAME".to_string(),
    ModuleType::Base64 => {
      let source = source.into_bytes();
      let encoded = rolldown_utils::base64::to_standard_base64(source);
      text_to_string_literal(&encoded)?
    }
    ModuleType::Dataurl => {
      let data = source.into_bytes();
      let guessed_mime = guess_mime(path, &data)?;
      let dataurl = rolldown_utils::dataurl::encode_as_shortest_dataurl(&guessed_mime, &data);
      text_to_string_literal(&dataurl)?
    }
    ModuleType::Binary => {
      let source = source.into_bytes();
      let encoded = rolldown_utils::base64::to_standard_base64(source);
      binary_to_esm(&encoded, options.platform, RUNTIME_MODULE_ID)
    }
    ModuleType::Empty => String::new(),
    ModuleType::Custom(custom_type) => {
      // TODO: should provide friendly error message to say that this type is not supported by rolldown.
      // Users should handle this type in load/transform hooks
      return Err(anyhow::format_err!("Unknown module type: {custom_type}"))?;
    }
  };

  Ok((has_lazy_export, source.into(), module_type.into()))
}
