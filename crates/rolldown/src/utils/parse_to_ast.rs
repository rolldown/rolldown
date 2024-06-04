use std::{path::Path, sync::Arc};

use oxc::span::SourceType as OxcSourceType;
use rolldown_common::{ModuleType, NormalizedBundlerOptions};
use rolldown_loader_utils::{json_to_esm, text_to_esm};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

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
  options: &NormalizedBundlerOptions,
  resource_id: &Path,
  source: impl Into<Arc<str>>,
) -> anyhow::Result<OxcAst> {
  let source: Arc<str> = source.into();

  // 1. Determine the ModuleType based on the file extension.
  let module_type = {
    let ext = resource_id.extension().and_then(|ext| ext.to_str()).unwrap_or("js");
    let module_type = options.module_types.get(ext);

    // FIXME: Once we support more types, we should return error instead of defaulting to JS.
    module_type.copied().unwrap_or(ModuleType::Js)
  };
  // 2. Transform the source to the type that rolldown supported.
  let (source, parsed_type) = match module_type {
    ModuleType::Js => (source, ParseType::Js),
    ModuleType::Json => (json_to_esm(&source)?.into(), ParseType::Js),
    ModuleType::Text => (text_to_esm(&source)?.into(), ParseType::Js),
  };

  // 3. Parse the source to AST and transform non-js AST to valid JS AST.
  let valid_js_ast = match parsed_type {
    ParseType::Js => OxcCompiler::parse(Arc::clone(&source), pure_esm_js_oxc_source_type())?,
    ParseType::Jsx => todo!(),
    ParseType::Ts => todo!(),
    ParseType::Tsx => todo!(),
  };

  Ok(valid_js_ast)
}
