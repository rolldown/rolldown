use std::{path::Path, sync::Arc};

use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

pub fn transform(path: &Path, source: Arc<str>, source_type: SourceType) -> anyhow::Result<OxcAst> {
  let mut ast = OxcCompiler::parse(source, source_type)?;
  if let Err(errors) = ast.with_mut(|fields| {
    let mut transformer_options = TransformOptions::default();
    if source_type.is_jsx() {
      transformer_options.react.jsx_plugin = true;
    }

    Transformer::new(
      fields.allocator,
      path,
      source_type,
      fields.source,
      fields.trivias,
      transformer_options,
    )
    .build(fields.program)
  }) {
    Err(anyhow::anyhow!("Transform failed, got {:#?}", errors))
  } else {
    Ok(ast)
  }
}
