use std::sync::Arc;

use rolldown_common::{
  AttachDebugInfo, ModuleRenderOutput, NormalModule, NormalizedBundlerOptions,
  SourcemapChainElement,
};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::{
  Source, SourceJoiner, SourceMap, SourceMapSource, collapse_sourcemaps_traced, empty_sourcemap,
};
use rolldown_utils::concat_string;

pub struct RenderEcmaModuleOutput {
  pub content: Option<RenderedModuleContent>,
  pub warnings: Vec<BuildDiagnostic>,
}

type SharedSources = Arc<[Box<dyn Source + Send + Sync>]>;

/// Code is shared with `RenderedModule`, while its map belongs only to chunk assembly.
/// See internal-docs/module-sourcemap-ownership/implementation.md.
pub struct RenderedModuleContent {
  sources: SharedSources,
  mapped_body: Option<Box<SourceMapSource<ModuleBody>>>,
}

/// A map's content shares the module's source pieces without retaining another map.
struct ModuleBody(SharedSources);

impl AsRef<str> for ModuleBody {
  fn as_ref(&self) -> &str {
    self.0.last().expect("a rendered module has a code body").as_ref().content()
  }
}

impl RenderedModuleContent {
  fn new(sources: Vec<Box<dyn Source + Send + Sync>>, map: Option<SourceMap>) -> Self {
    // The optional module marker precedes the code body, which is always last.
    debug_assert!(!sources.is_empty());
    let sources: SharedSources = Arc::from(sources.into_boxed_slice());
    let mapped_body = map.map(|map| {
      Box::new(
        SourceMapSource::new(ModuleBody(Arc::clone(&sources)), map)
          .with_pre_compute_sourcemap_data(true),
      )
    });
    Self { sources, mapped_body }
  }

  pub fn sources(&self) -> &SharedSources {
    &self.sources
  }

  pub fn append_to<'a>(&'a mut self, joiner: &mut SourceJoiner<'a>) {
    if let Some(mapped_body) = self.mapped_body.take() {
      for source in &self.sources[..self.sources.len() - 1] {
        joiner.append_source(source);
      }
      // Transfer the existing box; the joiner can take its map without another allocation.
      joiner.append_source_dyn(mapped_body);
    } else {
      for source in self.sources.iter() {
        joiner.append_source(source);
      }
    }
  }
}

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> RenderEcmaModuleOutput {
  if render_output.code.is_empty() {
    return RenderEcmaModuleOutput { content: None, warnings: vec![] };
  }
  let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = Vec::with_capacity(6);
  // Mark only where a module starts, like esbuild and bun.
  if matches!(
    options.experimental.attach_debug_info,
    Some(AttachDebugInfo::Simple | AttachDebugInfo::Full)
  ) {
    sources.push(Box::new(concat_string!("// ", module.debug_id)));
  }

  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  let mut warnings = vec![];

  // Because oxc codegen sourcemap is last of sourcemap chain,
  // If here no extra sourcemap need remapping, we using it as final module sourcemap.
  // So here make sure using correct `source_name` and `source_content.

  let map = if enable_sourcemap {
    collapse_module_sourcemap(
      &module.sourcemap_chain,
      render_output.map,
      module.id.as_str(),
      &mut warnings,
    )
  } else {
    None
  };
  sources.push(Box::new(render_output.code));

  RenderEcmaModuleOutput { content: Some(RenderedModuleContent::new(sources, map)), warnings }
}

/// Collapses a module's `sourcemap_chain` together with the oxc codegen map
/// (`codegen_map`, always the last element of the chain) into the final module
/// sourcemap. Returns `None` when there is no sourcemap to emit.
///
/// Like Rollup, the result keeps only the sources and names that a traced token
/// references. So a transform that returns `{ mappings: '' }` or omits its map
/// removes the module's earlier code, and the names that came with it.
///
/// A `SOURCEMAP_BROKEN` warning is pushed onto `warnings` for every plugin that
/// omitted its sourcemap, since that breaks the mapping chain for `module_id`.
pub fn collapse_module_sourcemap(
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
  // The collapsed map takes its sources from the first map.
  // The chain is not changed, because `this.getCombinedSourcemap()` reads it.
  let resolved_first = owned_chain
    .first()
    .and_then(|first| rolldown_sourcemap::resolve_sources_against_module(first, module_id));
  if let Some(resolved_first) = resolved_first.as_ref() {
    owned_chain[0] = resolved_first;
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
    Some(collapse_sourcemaps_traced(&owned_chain))
  }
}

#[cfg(test)]
mod tests {
  use super::{RenderedModuleContent, collapse_module_sourcemap};
  use arcstr::ArcStr;
  use insta::assert_snapshot;
  use rolldown_common::{PluginIdx, SourcemapChainElement};
  use rolldown_sourcemap::{SourceMap, SourceMapBuilder, empty_sourcemap};

  const MODULE_ID: &str = "/project/src/index.js";

  #[test]
  fn chunk_takes_map_strings_and_keeps_shared_module_code() {
    use rolldown_sourcemap::SourceJoiner;
    use std::sync::Arc;

    let module_map = SourceMap::from_json_string(
      r#"{"version":3,"names":["first"],"sources":[null,"/project/src/index.js"],"sourcesContent":[null,"first();\nsecond();"],"mappings":"ACAAA;AACA"}"#,
    ).unwrap().into_owned();
    let name_ptr = module_map.get_names().next().unwrap().as_ptr();
    let source_ptr = module_map.get_sources().nth(1).unwrap().unwrap().as_ptr();
    let content_ptr = module_map.get_source_content(1).unwrap().as_ptr();
    let code = "first();\nsecond();".to_string();
    let code_ptr = code.as_ptr();
    let mut content = RenderedModuleContent::new(
      vec![Box::new("// module".to_string()), Box::new(code)],
      Some(module_map),
    );
    let shared_sources = Arc::clone(content.sources());
    let mut following = RenderedModuleContent::new(
      vec![Box::new("third();".to_string())],
      Some(map("third.js", "third();")),
    );
    let mut joiner = SourceJoiner::default();
    joiner.prepend_source("// banner");
    content.append_to(&mut joiner);
    following.append_to(&mut joiner);
    joiner.append_source("// footer");
    let (code, map) = joiner.join();
    let map = map.unwrap();
    assert_eq!(code, "// banner\n// module\nfirst();\nsecond();\nthird();\n// footer");
    assert_eq!(map.get_names().next().unwrap().as_ptr(), name_ptr);
    assert_eq!(map.get_sources().nth(1).unwrap().unwrap().as_ptr(), source_ptr);
    assert_eq!(map.get_source_content(1).unwrap().as_ptr(), content_ptr);
    assert_eq!(map.get_sources().next(), Some(None));
    assert_eq!(map.get_source_content(0), None);
    assert_eq!(map.get_token(0).unwrap().get_dst_line(), 2);
    assert_eq!(map.get_token(1).unwrap().get_dst_line(), 3);
    assert_eq!(map.get_token(2).unwrap().get_dst_line(), 4);
    assert_eq!(shared_sources[1].as_ref().content().as_ptr(), code_ptr);
    assert!(shared_sources.iter().all(|source| source.sourcemap().is_none()));
    drop(joiner);
    assert!(content.mapped_body.is_none());
    drop(content);
    assert_eq!(Arc::strong_count(&shared_sources), 1);
  }

  #[test]
  fn unmapped_module_has_no_mapped_source() {
    use rolldown_sourcemap::SourceJoiner;
    let mut content = RenderedModuleContent::new(
      vec![Box::new("// module".to_string()), Box::new("run();".to_string())],
      None,
    );
    assert!(content.mapped_body.is_none());
    let mut joiner = SourceJoiner::default();
    content.append_to(&mut joiner);
    let (code, map) = joiner.join();
    assert_eq!(code, "// module\nrun();");
    assert!(map.is_none());
  }

  #[test]
  #[cfg(target_pointer_width = "64")]
  fn rendered_module_source_stays_compact() {
    use crate::ecmascript::ecma_generator::RenderedModuleSource;
    assert_eq!(size_of::<RenderedModuleSource>(), 32);
  }

  fn plugin_idx() -> PluginIdx {
    PluginIdx::from_usize(0)
  }

  /// Builds a sourcemap for `source`/`content` with two tokens at columns 0 and 6 of the
  /// first line. The tokens are identity mappings so they line up across maps and survive
  /// `collapse_sourcemaps`' lookups.
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
    // A real `Load`/`Transform` map wins over the module id placeholder: its sources, content
    // and mappings flow through the collapse.
    let mut warnings = vec![];
    let load = SourcemapChainElement::Load(map("/project/src/loaded.js", "const a = 1;\n"));
    let result =
      collapse_module_sourcemap(&[load], Some(codegen_map("a(1);\n")), MODULE_ID, &mut warnings)
        .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/loaded.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());
  }

  #[test]
  fn map_without_mappings_drops_earlier_sources_and_names() {
    // `{ mappings: '' }` or an omitted map after a real map leaves no token that reaches the
    // original code, so its source, content and names must not survive (#11176).
    let mut builder = SourceMapBuilder::default();
    let source_id = builder.add_source_and_content(MODULE_ID, "const secret = 1;\n");
    let name_id = builder.add_name("secret");
    builder.add_token(0, 6, 0, 6, Some(source_id), Some(name_id));
    let earlier =
      SourcemapChainElement::Transform((plugin_idx(), builder.into_sourcemap().into_owned()));
    for last in [SourcemapChainElement::Transform((plugin_idx(), empty_sourcemap())), omitted()] {
      let mut warnings = vec![];
      let result = collapse_module_sourcemap(
        &[earlier.clone(), last],
        Some(codegen_map("proxy();\n")),
        MODULE_ID,
        &mut warnings,
      )
      .unwrap();
      assert_eq!(result.to_json_string(), r#"{"version":3,"names":[],"sources":[],"mappings":""}"#);
    }
  }

  #[cfg(not(windows))]
  #[test]
  fn relative_sources_of_the_first_map_are_resolved_against_the_module() {
    let mut builder = SourceMapBuilder::default();
    let a = builder.add_source_and_content("../lib/a.ts", "const a = 1;\n");
    let b = builder.add_source_and_content("../lib/b.ts", "const b = 2;\n");
    builder.add_token(0, 0, 0, 0, Some(a), None);
    builder.add_token(0, 6, 0, 6, Some(b), None);
    let transform = builder.into_sourcemap().into_owned();
    let chain = [SourcemapChainElement::Transform((plugin_idx(), transform))];

    let mut warnings = vec![];
    let result = collapse_module_sourcemap(
      &chain,
      Some(codegen_map("const a = 1;\n")),
      MODULE_ID,
      &mut warnings,
    )
    .unwrap();
    assert_eq!(
      result.get_sources().collect::<Vec<_>>(),
      [Some("/project/lib/a.ts"), Some("/project/lib/b.ts")]
    );
    let SourcemapChainElement::Transform((_, transform)) = &chain[0] else { unreachable!() };
    assert_eq!(transform.get_source(0), Some("../lib/a.ts"));
  }
}
