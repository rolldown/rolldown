use std::sync::Arc;

use rolldown_common::{ModuleRenderOutput, NormalModule, NormalizedBundlerOptions};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::{Source, SourceMapSource, collapse_sourcemaps, empty_sourcemap};
use rolldown_utils::concat_string;

pub struct RenderEcmaModuleOutput {
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  /// `SOURCEMAP_BROKEN` warnings collected while collapsing this module's
  /// sourcemap chain. Empty unless the module had `Omitted` chain entries
  /// AND its code reached the chunk AND sourcemap output is enabled.
  pub warnings: Vec<BuildDiagnostic>,
}

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> RenderEcmaModuleOutput {
  if render_output.code.is_empty() {
    return RenderEcmaModuleOutput { sources: None, warnings: vec![] };
  }
  let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = Vec::with_capacity(6);
  if options.experimental.is_attach_debug_info_enabled() {
    sources.push(Box::new(concat_string!("//#region ", module.debug_id)));
  }

  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  let mut warnings = vec![];

  // Because oxc codegen sourcemap is last of sourcemap chain,
  // If here no extra sourcemap need remapping, we using it as final module sourcemap.
  // So here make sure using correct `source_name` and `source_content.

  if enable_sourcemap {
    let sourcemap = if module.sourcemap_chain.is_empty() {
      render_output.map
    } else {
      // Materialize a sentinel sourcemap for each `Omitted` entry so we can
      // pass `&SourceMap` references into `collapse_sourcemaps`. An empty
      // sourcemap drops all subsequent token lookups — mirrors Rollup's
      // empty-mappings `Link` for `{ missing: true }` entries.
      let empty = empty_sourcemap();
      let mut owned_chain: Vec<&rolldown_sourcemap::SourceMap> =
        Vec::with_capacity(module.sourcemap_chain.len() + 1);
      for element in &module.sourcemap_chain {
        match element {
          rolldown_common::SourcemapChainElement::Transform((_, sourcemap))
          | rolldown_common::SourcemapChainElement::Load(sourcemap) => {
            owned_chain.push(sourcemap);
          }
          rolldown_common::SourcemapChainElement::Omitted { plugin_name, .. } => {
            owned_chain.push(&empty);
            // Match Rollup: emit `SOURCEMAP_BROKEN` only when this module's
            // transformed code actually contributes to a chunk and sourcemap
            // output is enabled. Both are true here (`render_output.code` is
            // non-empty, `enable_sourcemap` is true).
            warnings.push(
              BuildDiagnostic::sourcemap_broken(
                plugin_name.to_string(),
                Some(module.id.to_string()),
              )
              .with_severity_warning(),
            );
          }
        }
      }
      if let Some(sourcemap) = render_output.map.as_ref() {
        owned_chain.push(sourcemap);
      }
      Some(collapse_sourcemaps(&owned_chain))
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

  RenderEcmaModuleOutput { sources: Some(Arc::from(sources.into_boxed_slice())), warnings }
}
