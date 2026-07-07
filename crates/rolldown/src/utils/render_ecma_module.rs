use std::sync::Arc;

use rolldown_common::{
  ModuleRenderOutput, NormalModule, NormalizedBundlerOptions, SourcemapChainElement,
};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::{
  Source, SourceMap, SourceMapSource, collapse_sourcemaps, empty_sourcemap,
};
use rolldown_utils::concat_string;

pub struct RenderEcmaModuleOutput {
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
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
    if let Some(sourcemap) = collapse_module_sourcemap(
      &module.sourcemap_chain,
      render_output.map,
      module.id.as_str(),
      &mut warnings,
    ) {
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

/// Collapses a module's `sourcemap_chain` together with the oxc codegen map
/// (`codegen_map`, always the last element of the chain) into the final module
/// sourcemap. Returns `None` when there is no sourcemap to emit.
///
/// A `SOURCEMAP_BROKEN` warning is pushed onto `warnings` for every plugin that
/// omitted its sourcemap, since that breaks the mapping chain for `module_id`.
fn collapse_module_sourcemap(
  sourcemap_chain: &[SourcemapChainElement],
  codegen_map: Option<SourceMap>,
  module_id: &str,
  warnings: &mut Vec<BuildDiagnostic>,
) -> Option<SourceMap> {
  if sourcemap_chain.is_empty() {
    return codegen_map;
  }

  let empty = empty_sourcemap();
  let mut owned_chain: Vec<&SourceMap> = Vec::with_capacity(sourcemap_chain.len() + 1);

  let mut original_content: Option<&str> = None;
  for element in sourcemap_chain {
    match element {
      SourcemapChainElement::Transform((_, sourcemap)) | SourcemapChainElement::Load(sourcemap) => {
        owned_chain.push(sourcemap);
      }
      SourcemapChainElement::Omitted { plugin_name, .. } => {
        owned_chain.push(&empty);
        warnings.push(
          BuildDiagnostic::sourcemap_broken(plugin_name.to_string(), Some(module_id.to_string()))
            .with_severity_warning(),
        );
      }
      SourcemapChainElement::Null { original_content: content, .. } => {
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
    codegen_map.map(|mut map| {
      if let Some(content) = original_content {
        map.set_source_contents(vec![Some(content)]);
      }
      map
    })
  } else {
    if let Some(sourcemap) = codegen_map.as_ref() {
      owned_chain.push(sourcemap);
    }
    Some(collapse_sourcemaps(&owned_chain))
  }
}

#[cfg(test)]
mod tests {
  use super::collapse_module_sourcemap;
  use arcstr::ArcStr;
  use insta::assert_snapshot;
  use rolldown_common::{PluginIdx, SourcemapChainElement};
  use rolldown_sourcemap::{SourceMap, SourceMapBuilder};

  const MODULE_ID: &str = "/project/src/index.js";

  fn plugin_idx() -> PluginIdx {
    PluginIdx::from_usize(0)
  }

  /// Builds a sourcemap for `source`/`content` with two tokens at columns 0 and 6 of the
  /// first line. The tokens are identity mappings so they line up across maps and survive
  /// `collapse_sourcemaps`' source-view lookups.
  fn map(source: &str, content: &str) -> SourceMap {
    let mut builder = SourceMapBuilder::default();
    let source_id = builder.add_source_and_content(source, content);
    builder.add_token(0, 0, 0, 0, Some(source_id), None);
    builder.add_token(0, 6, 0, 6, Some(source_id), None);
    builder.into_sourcemap().into_owned()
  }

  /// The oxc codegen map is always the last element of the chain and, in production, uses
  /// the module id as its source name (see `NormalModule::render`).
  fn codegen_map(content: &str) -> SourceMap {
    map(MODULE_ID, content)
  }

  fn omitted() -> SourcemapChainElement {
    SourcemapChainElement::Omitted { plugin_idx: plugin_idx(), plugin_name: ArcStr::from("plugin") }
  }

  fn null(original_content: &str) -> SourcemapChainElement {
    SourcemapChainElement::Null {
      plugin_idx: plugin_idx(),
      original_content: ArcStr::from(original_content),
    }
  }

  #[test]
  fn empty_chain_returns_codegen_map_unchanged() {
    let mut warnings = vec![];
    let result =
      collapse_module_sourcemap(&[], Some(codegen_map("const a = 1;\n")), MODULE_ID, &mut warnings)
        .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/index.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());

    assert!(collapse_module_sourcemap(&[], None, MODULE_ID, &mut warnings).is_none());
  }

  #[test]
  fn omitted_only_with_codegen_map_uses_module_id_as_source() {
    let mut warnings = vec![];
    let result = collapse_module_sourcemap(
      &[omitted()],
      Some(codegen_map("const a = 1;\n")),
      MODULE_ID,
      &mut warnings,
    )
    .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":[],"mappings":""}"#);
    // An omitted sourcemap breaks the chain, so a `SOURCEMAP_BROKEN` warning is emitted.
    assert_eq!(warnings.len(), 1);
  }

  #[test]
  fn null_only_with_codegen_map_swaps_source_content() {
    // `map: null` keeps the codegen map's positions/sources but swaps in the pre-transform
    // source content so the transformed code does not leak into `sourcesContent`.
    let mut warnings = vec![];
    let result = collapse_module_sourcemap(
      &[null("const a = 1;\n")],
      Some(codegen_map("a(1);\n")),
      MODULE_ID,
      &mut warnings,
    )
    .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/index.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());
  }

  #[test]
  fn null_only_without_codegen_map_returns_none() {
    let mut warnings = vec![];
    assert!(
      collapse_module_sourcemap(&[null("const a = 1;\n")], None, MODULE_ID, &mut warnings)
        .is_none()
    );
  }

  #[test]
  fn real_map_is_not_replaced_by_placeholder() {
    // A real `Load`/`Transform` map is kept as-is; its sources/content win over the module
    // id placeholder and its mappings flow through the collapse.
    let mut warnings = vec![];
    let load = SourcemapChainElement::Load(map("loaded.js", "const a = 1;\n"));
    let result =
      collapse_module_sourcemap(&[load], Some(codegen_map("a(1);\n")), MODULE_ID, &mut warnings)
        .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["loaded.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());
  }
}
