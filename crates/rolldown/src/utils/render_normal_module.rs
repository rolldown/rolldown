use std::sync::Arc;
// cSpell:disable

use rolldown_common::{NormalModule, RenderedModule};
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};
use rolldown_sourcemap::collapse_sourcemaps;

use crate::{types::module_render_output::ModuleRenderOutput, SharedOptions};

pub fn render_normal_module<'a>(
  module: &'a NormalModule,
  ast: &OxcProgram,
  source_name: &str,
  output_options: &SharedOptions,
) -> Option<ModuleRenderOutput<'a>> {
  if ast.program().body.is_empty() {
    None
  } else {
    let enable_sourcemap = !output_options.sourcemap.is_hidden() && !module.is_virtual;

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.
    let render_output = OxcCompiler::print(ast, source_name, enable_sourcemap);

    Some(ModuleRenderOutput {
      module_path: module.resource_id.expect_file().as_str(),
      module_pretty_path: &module.pretty_path,
      rendered_module: RenderedModule { code: None },
      rendered_content: render_output.source_text,
      sourcemap: if output_options.sourcemap.is_hidden() {
        None
      } else {
        let mut sourcemap_chain = module.sourcemap_chain.iter().map(Arc::clone).collect::<Vec<_>>();
        if let Some(sourcemap) = render_output.source_map {
          sourcemap_chain.push(Arc::new(sourcemap));
        }
        collapse_sourcemaps(sourcemap_chain, &output_options.dir)
      },
    })
  }
}
