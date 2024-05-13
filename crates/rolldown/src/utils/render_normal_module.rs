use rolldown_common::{NormalModule, RenderedModule};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};
use rolldown_sourcemap::{collapse_sourcemaps, lines_count};

use crate::{types::module_render_output::ModuleRenderOutput, SharedOptions};

pub fn render_normal_module<'a>(
  module: &'a NormalModule,
  ast: &OxcAst,
  source_name: &str,
  options: &SharedOptions,
) -> Option<ModuleRenderOutput<'a>> {
  if ast.is_body_empty() {
    None
  } else {
    let enable_sourcemap = !options.sourcemap.is_hidden() && !module.is_virtual;

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.
    let render_output = OxcCompiler::print(ast, source_name, enable_sourcemap);

    Some(ModuleRenderOutput {
      module_path: module.resource_id.clone(),
      module_pretty_path: &module.pretty_path,
      rendered_module: RenderedModule { code: None },
      // Search lines count from rendered content has a little overhead, so make it at parallel.
      lines_count: lines_count(&render_output.source_text),
      rendered_content: render_output.source_text,
      sourcemap: if options.sourcemap.is_hidden() {
        None
      } else if module.sourcemap_chain.is_empty() {
        render_output.source_map
      } else {
        let mut sourcemap_chain = module.sourcemap_chain.iter().collect::<Vec<_>>();
        if let Some(sourcemap) = render_output.source_map.as_ref() {
          sourcemap_chain.push(sourcemap);
        }
        collapse_sourcemaps(sourcemap_chain)
      },
    })
  }
}
