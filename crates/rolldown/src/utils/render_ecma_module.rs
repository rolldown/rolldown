use std::sync::Arc;

use rolldown_common::{ModuleRenderOutput, NormalModule, NormalizedBundlerOptions};
use rolldown_sourcemap::{Source, SourceMapSource, collapse_sourcemaps, empty_sourcemap};
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
        let empty = empty_sourcemap();
        let mut owned_chain: Vec<&rolldown_sourcemap::SourceMap> =
          Vec::with_capacity(module.sourcemap_chain.len() + 1);

        let mut original_content: Option<&str> = None;
        for element in &module.sourcemap_chain {
          match element {
            rolldown_common::SourcemapChainElement::Transform((_, sourcemap))
            | rolldown_common::SourcemapChainElement::Load(sourcemap) => {
              owned_chain.push(sourcemap);
            }
            rolldown_common::SourcemapChainElement::Omitted { .. } => {
              owned_chain.push(&empty);
            }
            rolldown_common::SourcemapChainElement::Null { original_content: content, .. } => {
              // `map: null` does not remap positions, so it contributes nothing
              // to `collapse_sourcemaps`. We only keep its pre-transform content as a
              // fallback when there is no real map to provide one.
              if original_content.is_none() {
                original_content = Some(content);
              }
            }
          }
        }
        if owned_chain.is_empty() {
          // Only `map: null` transforms touched this module: keep the codegen
          // map's positions but swap in the pre-transform source content so the
          // transformed/injected code does not leak into `sourcesContent`.
          render_output.map.map(|mut map| {
            if let Some(content) = original_content {
              map.set_source_contents(vec![Some(content)]);
            }
            map
          })
        } else {
          if let Some(sourcemap) = render_output.map.as_ref() {
            owned_chain.push(sourcemap);
          }
          Some(collapse_sourcemaps(&owned_chain))
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
