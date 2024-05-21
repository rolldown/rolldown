use std::{path::Path, sync::Arc};

use oxc::span::SourceType as OxcSourceType;
use rolldown_common::Loader;
use rolldown_lang_json::json_to_esm;
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

fn pure_esm_js_oxc_source_type() -> OxcSourceType {
  let pure_esm_js = OxcSourceType::default().with_module(true);
  debug_assert!(pure_esm_js.is_javascript());
  debug_assert!(!pure_esm_js.is_jsx());
  debug_assert!(pure_esm_js.is_module());
  debug_assert!(pure_esm_js.is_strict());

  pure_esm_js
}

pub fn parse_to_ast(resource_id: &Path, source: impl Into<Arc<str>>) -> anyhow::Result<OxcAst> {
  let source: Arc<str> = source.into();

  let loader =
    resource_id.extension().and_then(|ext| ext.to_str()).map_or(Loader::Js, |ext| match ext {
      "json" => Loader::Json,
      _ => Loader::Js,
    });

  let ast = match loader {
    Loader::Js => OxcCompiler::parse(Arc::clone(&source), pure_esm_js_oxc_source_type()),
    Loader::Json => {
      let source: Arc<str> = json_to_esm(&source)?.into();
      OxcCompiler::parse(Arc::clone(&source), pure_esm_js_oxc_source_type())
    }
  };

  Ok(ast)
}
