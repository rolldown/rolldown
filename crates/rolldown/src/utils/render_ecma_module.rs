use rolldown_common::{EcmaModule, NormalizedBundlerOptions};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_sourcemap::{collapse_sourcemaps, lines_count, RawSource, Source, SourceMapSource};

pub fn render_ecma_module(
  module: &EcmaModule,
  ast: &EcmaAst,
  source_name: &str,
  options: &NormalizedBundlerOptions,
) -> Option<Vec<Box<dyn Source + Send>>> {
  if ast.is_body_empty() {
    None
  } else {
    let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send>> = vec![];
    sources.push(Box::new(RawSource::new(format!(
      "//#region {debug_module_id}",
      debug_module_id = module.debug_id
    ))));

    let enable_sourcemap = !options.sourcemap.is_hidden() && !module.is_virtual();

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.
    let render_output = EcmaCompiler::print(ast, source_name, enable_sourcemap);

    if enable_sourcemap {
      let sourcemap = if module.sourcemap_chain.is_empty() {
        render_output.source_map
      } else {
        let mut sourcemap_chain = module.sourcemap_chain.iter().collect::<Vec<_>>();
        if let Some(sourcemap) = render_output.source_map.as_ref() {
          sourcemap_chain.push(sourcemap);
        }
        Some(collapse_sourcemaps(sourcemap_chain))
      };

      if let Some(sourcemap) = sourcemap {
        let lines_count = lines_count(&render_output.source_text);
        sources.push(Box::new(SourceMapSource::new(
          render_output.source_text,
          sourcemap,
          lines_count,
        )));
      } else {
        sources.push(Box::new(RawSource::new(render_output.source_text)));
      }
    } else {
      sources.push(Box::new(RawSource::new(render_output.source_text)));
    }

    sources.push(Box::new(RawSource::new("//#endregion".to_string())));

    Some(sources)
  }
}
