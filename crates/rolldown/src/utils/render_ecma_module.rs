use std::sync::Arc;

use rolldown_common::{ModuleRenderOutput, NormalModule, NormalizedBundlerOptions};
use rolldown_sourcemap::{Source, SourceMapSource, collapse_sourcemaps};
use rolldown_utils::concat_string;

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> Option<Arc<[Box<dyn Source + Send + Sync>]>> {
  if render_output.code.is_empty() {
    None
  } else {
    let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = vec![];
    sources.push(Box::new(concat_string!("//#region ", module.debug_id)));

    let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.

    if enable_sourcemap {
      let sourcemap = if module.sourcemap_chain.is_empty() {
        render_output.map
      } else {
        let mut sourcemap_chain = module.sourcemap_chain.iter().collect::<Vec<_>>();
        if let Some(sourcemap) = render_output.map.as_ref() {
          sourcemap_chain.push(sourcemap);
        }
        Some(collapse_sourcemaps(sourcemap_chain))
      };

      if let Some(sourcemap) = sourcemap {
        sources.push(Box::new(
          SourceMapSource::new(render_output.code, sourcemap)
            .with_pre_compute_sourcemap_data(options.is_sourcemap_enabled()),
        ));
      } else {
        sources.push(Box::new(render_output.code.trim_end().to_string()));
      }
    } else {
      sources.push(Box::new(render_output.code.trim_end().to_string()));
    }

    sources.push(Box::new("//#endregion\n"));

    Some(Arc::from(sources.into_boxed_slice()))
  }
}
