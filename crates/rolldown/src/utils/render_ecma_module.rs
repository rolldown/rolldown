use std::sync::Arc;

use rolldown_common::{
  ModuleRenderOutput, NormalModule, NormalizedBundlerOptions, SourcemapChainElement,
};
use rolldown_sourcemap::{
  Source, SourceMapSource, anchor_sourcemap_to_source, collapse_sourcemaps,
};
use rolldown_utils::concat_string;

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> Option<Arc<[Box<dyn Source + Send + Sync>]>> {
  if render_output.code.is_empty() {
    None
  } else {
    let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = Vec::with_capacity(6);
    if options.experimental.is_attach_debug_info_enabled() {
      sources.push(Box::new(concat_string!("//#region ", module.debug_id)));
    }

    let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();

    // Because oxc codegen sourcemap is last of sourcemap chain,
    // If here no extra sourcemap need remapping, we using it as final module sourcemap.
    // So here make sure using correct `source_name` and `source_content.

    if enable_sourcemap {
      let sourcemap = if module.sourcemap_chain.is_empty() {
        render_output.map
      } else {
        // `Identity` elements (transforms that changed code without a sourcemap)
        // carry no map; collapse only the real maps that surround them.
        let mut sourcemap_chain = module
          .sourcemap_chain
          .iter()
          .filter_map(|element| match element {
            SourcemapChainElement::Transform((_, sourcemap))
            | SourcemapChainElement::Load(sourcemap) => Some(sourcemap),
            SourcemapChainElement::Identity { .. } => None,
          })
          .collect::<Vec<_>>();
        if let Some(sourcemap) = render_output.map.as_ref() {
          sourcemap_chain.push(sourcemap);
        }
        let collapsed = match sourcemap_chain.len() {
          0 => None,
          1 => Some(sourcemap_chain[0].clone()),
          _ => Some(collapse_sourcemaps(&sourcemap_chain)),
        };
        // When the chain starts with an `Identity` layer, the collapsed map was
        // traced through the real maps only and lost the original source. Re-anchor
        // it to the original module source (content + original line bounds).
        match (collapsed, module.sourcemap_chain.first()) {
          (Some(map), Some(SourcemapChainElement::Identity { original_code, .. })) => {
            Some(anchor_sourcemap_to_source(&map, module.id.as_str(), original_code))
          }
          (collapsed, _) => collapsed,
        }
      };

      if let Some(sourcemap) = sourcemap {
        sources.push(Box::new(
          SourceMapSource::new(render_output.code, sourcemap)
            .with_pre_compute_sourcemap_data(options.is_sourcemap_enabled()),
        ));
      } else {
        sources.push(Box::new(render_output.code));
      }
    } else {
      sources.push(Box::new(render_output.code));
    }

    if options.experimental.is_attach_debug_info_enabled() {
      sources.push(Box::new("//#endregion"));
    }

    Some(Arc::from(sources.into_boxed_slice()))
  }
}
