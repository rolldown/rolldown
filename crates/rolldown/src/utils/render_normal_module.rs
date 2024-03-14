use crate::types::module_render_context::ModuleRenderContext;
use oxc::codegen::CodegenReturn;
use rolldown_common::NormalModule;
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};
use rolldown_sourcemap::{SourceMap, SourceMapBuilder};
use string_wizard::MagicString;

pub struct RenderedNormalModuleOutput {
  pub code: MagicString<'static>,
  pub sourcemap: Option<SourceMap>,
}

#[allow(
  clippy::unnecessary_wraps,
  clippy::cast_possible_truncation,
  clippy::needless_pass_by_value
)]
pub fn render_normal_module(
  module: &NormalModule,
  _ctx: &ModuleRenderContext<'_>,
  ast: &OxcProgram,
  enable_sourcemap: Option<String>,
) -> Option<RenderedNormalModuleOutput> {
  if ast.program().body.is_empty() {
    None
  } else {
    let CodegenReturn { source_map, source_text } =
      OxcCompiler::print(ast, enable_sourcemap.clone());

    let mut source = MagicString::new(source_text);

    source.prepend(format!("// {}\n", module.pretty_path));

    // Here `MagicString` sourcemap is not valid, because it need to include valid ast token.

    Some(RenderedNormalModuleOutput {
      code: source,
      sourcemap: source_map.map(|source_map| {
        let mut sourcemap_builder = SourceMapBuilder::new(None);

        for (id, source) in source_map.sources().enumerate() {
          let source_id = sourcemap_builder.add_source(source);
          sourcemap_builder
            .set_source_contents(source_id, source_map.get_source_contents(id as u32));
        }

        for token in source_map.tokens() {
          sourcemap_builder.add(
            token.get_dst_line() + 1, // line offset by prepend comment
            token.get_dst_col(),
            token.get_src_line(),
            token.get_src_col(),
            token.get_source(),
            token.get_name(),
          );
        }

        sourcemap_builder.into_sourcemap()
      }),
    })
  }
}
