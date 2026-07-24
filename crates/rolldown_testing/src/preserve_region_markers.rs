//! Test-harness plugin that keeps `attachDebugInfo` region markers intact through rolldown's
//! default `dce-only` minify pass.
//!
//! Region markers are ordinary line comments, and oxc attaches a comment to the statement that
//! follows it (blank lines don't detach it). When the dce pass removes that statement, its
//! comments go with it — a removed statement at a module boundary silently takes the adjacent
//! `//#endregion` + `//#region` pair out of the output. That corrupts the region structure the
//! snapshot tooling relies on: the runtime-hide replacement matches `//#region … //#endregion`,
//! so an eaten pair at the runtime boundary makes snapshots silently hide user code.
//!
//! Legal comments survive the removal of their host statement, so this plugin disguises the
//! markers as legal comments right before the internal minify pass (`renderChunk` runs before it)
//! and restores them after everything else (`generateBundle`). Both rewrites are line-anchored
//! and reversible; codegen prints comments verbatim, so only the disguise prefix ever changes.
//! The plugin is registered after every fixture-defined plugin, so a fixture's own
//! `generateBundle` hook runs before the restore and observes the disguised `//!#region` form.
//!
//! This lives in the test harness on purpose: production builds and the public API are
//! untouched. The proper long-term fix is for oxc to re-attach orphaned comments of removed
//! statements, at which point this plugin can be deleted.

use std::borrow::Cow;
use std::sync::Arc;

use rolldown::plugin::{
  HookGenerateBundleArgs, HookNoopReturn, HookRenderChunkArgs, HookRenderChunkOutput,
  HookRenderChunkReturn, HookTransformOutputMap, HookUsage, Plugin, PluginContext,
};
use rolldown_common::{MinifyOptions, Output};

#[derive(Debug)]
pub struct PreserveRegionMarkersPlugin;

impl Plugin for PreserveRegionMarkersPlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown-testing:preserve-region-markers".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk | HookUsage::GenerateBundle
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    // Only the dce-only pass eats markers selectively; full minification strips all normal
    // comments on purpose, and a legal-comment disguise would wrongly keep markers alive there.
    if !matches!(args.options.minify, MinifyOptions::DeadCodeEliminationOnly(_)) {
      return Ok(None);
    }
    if !args.code.contains("//#region") {
      return Ok(None);
    }
    Ok(Some(HookRenderChunkOutput {
      code: map_region_marker_lines(&args.code, "//#", "//!#"),
      // The rewrite only touches whole-line comments, so the existing sourcemap stays valid —
      // `Null` states that on purpose (an omitted map would raise SOURCEMAP_BROKEN).
      map: HookTransformOutputMap::Null,
    }))
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    // Same gate as `render_chunk`: only a dce-only build can carry disguised markers, and a
    // fixture's own `//!#region`-shaped content must not be rewritten on other builds.
    if !matches!(args.options.minify, MinifyOptions::DeadCodeEliminationOnly(_)) {
      return Ok(());
    }
    for output in args.bundle.iter_mut() {
      if let Output::Chunk(chunk) = output {
        if chunk.code.contains("//!#") {
          Arc::make_mut(chunk).code = map_region_marker_lines(&chunk.code, "//!#", "//#");
        }
      }
    }
    Ok(())
  }
}

/// Rewrite the marker token of every line that starts (after indentation) with
/// `{from}region` or `{from}endregion`. Line-anchored, so `//# sourceMappingURL` and marker-like
/// text inside string content never match; only the first token on the line is touched.
fn map_region_marker_lines(source: &str, from: &str, to: &str) -> String {
  let mut out = String::with_capacity(source.len() + 64);
  for line in source.split_inclusive('\n') {
    let trimmed = line.trim_start();
    if trimmed.starts_with(from)
      && (trimmed[from.len()..].starts_with("region")
        || trimmed[from.len()..].starts_with("endregion"))
    {
      out.push_str(&line[..line.len() - trimmed.len()]);
      out.push_str(to);
      out.push_str(&trimmed[from.len()..]);
    } else {
      out.push_str(line);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::map_region_marker_lines;

  #[test]
  fn rewrite_is_line_anchored_and_reversible() {
    let source = "//#region a.js\n\tvar a = 1;\n\t//#endregion\nconst s = \"//#region not a marker\";\n//# sourceMappingURL=x.map\n";
    let protected = map_region_marker_lines(source, "//#", "//!#");
    assert_eq!(
      protected,
      "//!#region a.js\n\tvar a = 1;\n\t//!#endregion\nconst s = \"//#region not a marker\";\n//# sourceMappingURL=x.map\n"
    );
    assert_eq!(map_region_marker_lines(&protected, "//!#", "//#"), source);
  }
}
