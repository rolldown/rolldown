use crate::types::module_render_context::ModuleRenderContext;
use oxc::codegen::CodegenReturn;
use rolldown_common::NormalModule;
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};
use rolldown_sourcemap::SourceMap;
use string_wizard::MagicString;
use string_wizard::SourceMapOptions;

pub struct RenderedNormalModuleOutput {
  pub code: MagicString<'static>,
  pub sourcemap_chain: Vec<SourceMap>,
}

#[allow(clippy::unnecessary_wraps)]
pub fn render_normal_module(
  module: &NormalModule,
  _ctx: &ModuleRenderContext<'_>,
  ast: &OxcProgram,
  enable_sourcemap: Option<String>,
) -> Option<RenderedNormalModuleOutput> {
  if ast.program().body.is_empty() {
    None
  } else {
    let mut sourcemap_chain = vec![];
    let CodegenReturn { source_map, source_text } =
      OxcCompiler::print(ast, enable_sourcemap.clone());
    if let Some(source_map) = source_map {
      sourcemap_chain.push(source_map);
    }
    let mut source = MagicString::new(source_text);

    source.prepend(format!("// {}\n", module.pretty_path));

    if enable_sourcemap.is_some() {
      sourcemap_chain.push(source.source_map(SourceMapOptions { include_content: true }));
    }

    Some(RenderedNormalModuleOutput { code: source, sourcemap_chain })
  }
}
