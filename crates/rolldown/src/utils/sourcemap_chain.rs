//! Frees the memory of the module sourcemap chains in builds that are not incremental.
//!
//! Each `load` and `transform` hook that returns a map adds an element to the
//! `sourcemap_chain` of the module. Rendering composes the chain with the codegen map into the
//! final map of the module (`collapse_module_sourcemap`). Until then, the chain keeps every
//! intermediate map. In a large build, the chains can use more than 1 GiB.
//!
//! An incremental build (watch mode with `experimental.incrementalBuild`, and HMR) reads the
//! chains of unchanged modules in later builds. So it must not call these functions.

use std::sync::Arc;

use rolldown_common::{Module, ModuleTable, PluginIdx, SourcemapChainElement};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

/// Composes each run of consecutive `Transform` maps in every module chain into one map.
///
/// Map composition is associative, so rendering builds the same final map from the shorter
/// chain. The intermediate maps are freed before the link and generate stages.
pub fn collapse_sourcemap_chains(module_table: &mut ModuleTable) {
  module_table.modules.raw.par_iter_mut().for_each(|module| {
    let Module::Normal(module) = module else { return };
    if !has_transform_run(&module.sourcemap_chain) {
      return;
    }
    let chain = Arc::make_mut(&mut module.sourcemap_chain);
    *chain = collapse_transform_runs(std::mem::take(chain));
  });
}

/// Drops every module chain. Call it only after every module is rendered.
pub fn free_sourcemap_chains(module_table: &mut ModuleTable) {
  module_table.modules.raw.par_iter_mut().for_each(|module| {
    if let Module::Normal(module) = module {
      module.sourcemap_chain = Arc::default();
    }
  });
}

fn has_transform_run(chain: &[SourcemapChainElement]) -> bool {
  chain.windows(2).any(|pair| {
    matches!(pair, [SourcemapChainElement::Transform(_), SourcemapChainElement::Transform(_)])
  })
}

/// `Load`, `Omitted`, and `Null` elements stay in place and end a run. So rendering sees the
/// same sequence of elements, resolves the sources of the same first map, and emits the same
/// `SOURCEMAP_BROKEN` warnings. A composed run keeps the index of its last plugin. Nothing reads
/// the index after the scan stage sorts the chain by plugin order.
fn collapse_transform_runs(chain: Vec<SourcemapChainElement>) -> Vec<SourcemapChainElement> {
  let mut collapsed = Vec::with_capacity(chain.len());
  let mut run: Vec<(PluginIdx, SourceMap)> = Vec::new();
  for element in chain {
    match element {
      SourcemapChainElement::Transform(transform) => run.push(transform),
      other => {
        push_run(&mut run, &mut collapsed);
        collapsed.push(other);
      }
    }
  }
  push_run(&mut run, &mut collapsed);
  collapsed
}

fn push_run(run: &mut Vec<(PluginIdx, SourceMap)>, collapsed: &mut Vec<SourcemapChainElement>) {
  match run.len() {
    0 => {}
    1 => collapsed.extend(run.drain(..).map(SourcemapChainElement::Transform)),
    _ => {
      let maps: Vec<&SourceMap> = run.iter().map(|(_, map)| map).collect();
      let map = collapse_sourcemaps(&maps);
      let plugin_idx = run[run.len() - 1].0;
      collapsed.push(SourcemapChainElement::Transform((plugin_idx, map)));
      run.clear();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::collapse_transform_runs;
  use crate::utils::render_ecma_module::collapse_module_sourcemap;
  use arcstr::ArcStr;
  use rolldown_common::{PluginIdx, SourcemapChainElement};
  use rolldown_sourcemap::{SourceMap, SourceMapBuilder, empty_sourcemap};

  const MODULE_ID: &str = "/project/src/index.js";

  fn transform(map: SourceMap) -> SourcemapChainElement {
    SourcemapChainElement::Transform((PluginIdx::from_usize(0), map))
  }

  /// A map from `source` to the next code. Each token is (generated column, original column,
  /// name) on the first line.
  fn map(source: &str, content: &str, tokens: &[(u32, u32, Option<&str>)]) -> SourceMap {
    let mut builder = SourceMapBuilder::default();
    let source_id = builder.add_source_and_content(source, content);
    for &(dst_col, src_col, name) in tokens {
      let name_id = name.map(|name| builder.add_name(name));
      builder.add_token(0, dst_col, 0, src_col, Some(source_id), name_id);
    }
    builder.into_sourcemap().into_owned()
  }

  /// A transform map. Each transform moves the code, so a wrong composition shows in the result.
  fn shift(tokens: &[(u32, u32, Option<&str>)]) -> SourcemapChainElement {
    transform(map(MODULE_ID, "const value = 1;\n", tokens))
  }

  fn omitted() -> SourcemapChainElement {
    SourcemapChainElement::Omitted {
      plugin_idx: PluginIdx::from_usize(0),
      plugin_name: ArcStr::from("plugin"),
    }
  }

  fn null() -> SourcemapChainElement {
    SourcemapChainElement::Null {
      plugin_idx: PluginIdx::from_usize(0),
      original_content: ArcStr::from("const value = 1;\n"),
    }
  }

  /// The final module map as JSON, and the number of warnings.
  fn final_map(chain: &[SourcemapChainElement]) -> (Option<String>, usize) {
    let mut warnings = vec![];
    let codegen = map(MODULE_ID, "value(1);\n", &[(0, 0, None), (2, 3, None), (7, 9, None)]);
    let result = collapse_module_sourcemap(chain, Some(codegen), MODULE_ID, &mut warnings);
    (result.map(|map| map.to_json_string()), warnings.len())
  }

  /// Collapses `chain`, checks that rendering builds the same final map, and returns the
  /// collapsed chain.
  fn collapse_and_compare(chain: Vec<SourcemapChainElement>) -> Vec<SourcemapChainElement> {
    let expected = final_map(&chain);
    let collapsed = collapse_transform_runs(chain);
    assert_eq!(final_map(&collapsed), expected);
    collapsed
  }

  fn kinds(chain: &[SourcemapChainElement]) -> Vec<&'static str> {
    chain
      .iter()
      .map(|element| match element {
        SourcemapChainElement::Transform(_) => "transform",
        SourcemapChainElement::Load(_) => "load",
        SourcemapChainElement::Omitted { .. } => "omitted",
        SourcemapChainElement::Null { .. } => "null",
      })
      .collect()
  }

  #[test]
  fn consecutive_transforms_become_one() {
    let chain = vec![
      shift(&[(0, 0, None), (4, 6, Some("value")), (10, 14, None)]),
      shift(&[(0, 0, None), (3, 4, None), (9, 10, None)]),
      shift(&[(0, 0, None), (9, 3, None), (12, 9, None)]),
    ];
    assert_eq!(kinds(&collapse_and_compare(chain)), ["transform"]);
  }

  #[test]
  fn a_single_transform_is_kept() {
    let chain = vec![shift(&[(0, 0, None), (4, 6, None)])];
    assert_eq!(kinds(&collapse_and_compare(chain)), ["transform"]);
  }

  #[test]
  fn load_stays_and_the_transforms_after_it_become_one() {
    let load = SourcemapChainElement::Load(map(
      "/project/src/loaded.ts",
      "const value: number = 1;\n",
      &[(0, 0, None), (6, 6, Some("value")), (14, 22, None)],
    ));
    let chain = vec![
      load,
      shift(&[(0, 0, None), (4, 6, None), (10, 14, None)]),
      shift(&[(0, 0, None), (3, 4, None), (9, 10, None)]),
    ];
    assert_eq!(kinds(&collapse_and_compare(chain)), ["load", "transform"]);
  }

  #[test]
  fn omitted_ends_a_run_and_keeps_its_warning() {
    let chain = vec![
      shift(&[(0, 0, None), (4, 6, None)]),
      omitted(),
      shift(&[(0, 0, None), (3, 4, None)]),
      shift(&[(0, 0, None), (9, 3, None)]),
    ];
    let collapsed = collapse_and_compare(chain);
    assert_eq!(kinds(&collapsed), ["transform", "omitted", "transform"]);
    assert_eq!(final_map(&collapsed).1, 1);
  }

  #[test]
  fn null_ends_a_run() {
    let chain = vec![
      shift(&[(0, 0, None), (4, 6, None)]),
      shift(&[(0, 0, None), (3, 4, None)]),
      null(),
      shift(&[(0, 0, None), (9, 3, None)]),
      shift(&[(0, 0, None), (2, 9, None)]),
    ];
    assert_eq!(kinds(&collapse_and_compare(chain)), ["transform", "null", "transform"]);
  }

  #[test]
  fn names_of_an_earlier_map_survive() {
    let chain = vec![
      shift(&[(0, 0, Some("first")), (4, 6, Some("value")), (10, 14, None)]),
      shift(&[(0, 0, Some("renamed")), (3, 4, None), (9, 10, None)]),
    ];
    collapse_and_compare(chain);
  }

  #[test]
  fn a_map_without_mappings_still_drops_the_earlier_sources_and_names() {
    // The same case as `map_without_mappings_drops_earlier_sources_and_names` in
    // `render_ecma_module`: no token reaches the original code (#11176).
    let chain =
      vec![shift(&[(0, 6, Some("secret"))]), transform(empty_sourcemap()), shift(&[(0, 0, None)])];
    let collapsed = collapse_and_compare(chain);
    assert_eq!(kinds(&collapsed), ["transform"]);
  }

  #[cfg(not(windows))]
  #[test]
  fn relative_sources_of_the_first_map_are_still_resolved() {
    let first = transform(map("../lib/a.ts", "const value = 1;\n", &[(0, 0, None), (4, 6, None)]));
    let chain = vec![first, shift(&[(0, 0, None), (3, 4, None)])];
    let collapsed = collapse_and_compare(chain);
    let (json, _) = final_map(&collapsed);
    assert!(json.unwrap().contains(r#""sources":["/project/lib/a.ts"]"#));
  }
}
