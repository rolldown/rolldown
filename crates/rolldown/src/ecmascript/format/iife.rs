use crate::ecmascript::ecma_generator::RenderedModuleSources;
use crate::ecmascript::format::utils::wrapper::{render_wrapper_function, Injections};
use crate::types::generator::GenerateContext;
use rolldown_error::DiagnosableResult;
use rolldown_sourcemap::ConcatSource;

pub fn render_iife(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> DiagnosableResult<ConcatSource> {
  let injections = Injections { banner, footer, intro, outro };

  render_wrapper_function(ctx, module_sources, injections)
}
