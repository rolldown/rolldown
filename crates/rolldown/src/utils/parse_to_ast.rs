use std::{path::Path, sync::Arc};

use oxc::span::SourceType as OxcSourceType;
use rolldown_common::{Loader, NormalizedBundlerOptions};
use rolldown_lang_json::json_to_esm;
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

use super::text_to_esm::text_to_esm;

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

  // 1. Determine the loader based on the file extension.
  let loader = {
    let ext = resource_id.extension().and_then(|ext| ext.to_str()).unwrap_or("js");
    let loader = options.loaders.get(ext);

    // FIXME: Once we support more loaders, we should return error instead of defaulting to JS.
    loader.copied().unwrap_or(Loader::Js)
  };
  // 2. Transform the source to the type that rolldown supported.
  let (source, parsed_type) = match loader {
    Loader::Js => (source, ParseType::Js),
    Loader::Json => (json_to_esm(&source)?.into(), ParseType::Js),
    Loader::Text => (text_to_esm(&source)?.into(), ParseType::Js),
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
