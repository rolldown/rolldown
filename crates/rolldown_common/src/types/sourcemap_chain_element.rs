use arcstr::ArcStr;
use rolldown_sourcemap::{PackedSourceMap, SourceMap};

use crate::{NormalizedBundlerOptions, PluginIdx};

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A string representing the URL of an external source map.
  Transform((PluginIdx, SourceMap)),
  /// An inline source map represented as a JSON string.
  Load(SourceMap),
  /// A transform hook returned changed code without a sourcemap.
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
  /// A transform hook returned changed code together with an explicit
  /// `map: null`.
  Null { plugin_idx: PluginIdx, original_content: ArcStr },
}

/// A [`SourcemapChainElement`] with its map packed.
///
/// A module keeps its chain from the scan until it is rendered, and reads it once. Holding
/// the maps packed takes about 3 bytes for each token instead of 24.
#[derive(Debug, Clone)]
pub enum PackedSourcemapChainElement {
  Transform((PluginIdx, PackedSourceMap)),
  Load(PackedSourceMap),
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
  Null { plugin_idx: PluginIdx, original_content: ArcStr },
}

impl PackedSourcemapChainElement {
  /// Pack the elements of a chain that is complete, for a module to hold.
  ///
  /// Nothing reads the chain when the output has no sourcemap, so nothing is kept. And
  /// with `sourcemapExcludeSources`, the output drops the contents of the sources, so they are
  /// dropped here instead of held and copied through every collapse.
  pub fn pack_chain(
    chain: impl IntoIterator<Item = SourcemapChainElement>,
    options: &NormalizedBundlerOptions,
  ) -> Vec<Self> {
    if !options.is_sourcemap_enabled() {
      return Vec::new();
    }
    chain
      .into_iter()
      .map(|mut element| {
        if options.sourcemap_exclude_sources
          && let SourcemapChainElement::Transform((_, map)) | SourcemapChainElement::Load(map) =
            &mut element
        {
          map.set_source_contents(vec![]);
        }
        Self::from(element)
      })
      .collect()
  }
}

impl From<SourcemapChainElement> for PackedSourcemapChainElement {
  fn from(element: SourcemapChainElement) -> Self {
    match element {
      SourcemapChainElement::Transform((plugin_idx, map)) => {
        Self::Transform((plugin_idx, map.into_packed()))
      }
      SourcemapChainElement::Load(map) => Self::Load(map.into_packed()),
      SourcemapChainElement::Omitted { plugin_idx, plugin_name } => {
        Self::Omitted { plugin_idx, plugin_name }
      }
      SourcemapChainElement::Null { plugin_idx, original_content } => {
        Self::Null { plugin_idx, original_content }
      }
    }
  }
}
