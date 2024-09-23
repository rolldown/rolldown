use oxc::codegen::CodegenReturn;
use rolldown_common::{NormalModule, NormalizedBundlerOptions};
use rolldown_sourcemap::{collapse_sourcemaps, lines_count, RawSource, Source, SourceMapSource};

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: CodegenReturn,
) -> Option<Vec<Box<dyn Source + Send>>> {
  if render_output.source_text.is_empty() {
    None
  } else {
    let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send>> = vec![];
    sources.push(Box::new(RawSource::new(format!(
      "//#region {debug_module_id}",
      debug_module_id = module.debug_id
    ))));

    let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.

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
