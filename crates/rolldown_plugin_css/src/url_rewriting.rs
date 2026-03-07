use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, StrOrBytes};
use rolldown_plugin::PluginContext;
use rolldown_utils::dashmap::FxDashMap;

// ---------------------------------------------------------------------------
// Shared state type stored in `PluginContextMeta`
// ---------------------------------------------------------------------------

/// Mapping from lightningcss url() placeholder → emitted asset reference ID.
#[derive(Debug, Default)]
pub struct UrlPlaceholders {
  pub inner: FxDashMap<String, ArcStr>,
}

// ---------------------------------------------------------------------------
// URL extraction
// ---------------------------------------------------------------------------

/// A resolved url() dependency extracted from CSS.
#[derive(Debug)]
pub struct ResolvedUrlDep {
  /// The lightningcss placeholder string that replaced the url() in CSS output.
  pub placeholder: String,
  /// The absolute path to the referenced asset on disk.
  pub resolved_path: PathBuf,
  /// Optional fragment identifier (e.g. `#icon`) to append after the output path.
  pub fragment: Option<String>,
}

/// Parse CSS with `analyze_dependencies` to extract url() references, then
/// return the CSS with placeholders and the list of resolved dependencies.
///
/// URLs that are skipped (returned in the CSS as-is):
/// - `data:` URLs
/// - Absolute URLs (`http://`, `https://`, `//`)
pub fn extract_url_dependencies(
  css: &str,
  file_id: &str,
) -> anyhow::Result<(String, Vec<ResolvedUrlDep>)> {
  let file_path = Path::new(file_id);
  let base_dir = file_path.parent().unwrap_or(Path::new("."));

  let stylesheet = lightningcss::stylesheet::StyleSheet::parse(
    css,
    lightningcss::stylesheet::ParserOptions { filename: file_id.to_owned(), ..Default::default() },
  )
  .map_err(|e| anyhow::anyhow!("CSS parse error in {file_id}: {e}"))?;

  let result = stylesheet
    .to_css(lightningcss::printer::PrinterOptions {
      analyze_dependencies: Some(lightningcss::dependencies::DependencyOptions {
        remove_imports: false,
      }),
      ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("CSS printer error: {e}"))?;

  let mut resolved_deps = Vec::new();

  if let Some(deps) = &result.dependencies {
    for dep in deps {
      if let lightningcss::dependencies::Dependency::Url(url_dep) = dep {
        let url = &url_dep.url;

        // Skip data: URLs, absolute URLs, and protocol-relative URLs
        if url.starts_with("data:")
          || url.starts_with("http://")
          || url.starts_with("https://")
          || url.starts_with("//")
        {
          continue;
        }

        // Split off fragment identifier
        let (url_path, fragment) = match url.find('#') {
          Some(idx) => (&url[..idx], Some(url[idx..].to_owned())),
          None => (url.as_str(), None),
        };

        // Resolve relative to the CSS file's directory
        let resolved = base_dir.join(url_path);

        resolved_deps.push(ResolvedUrlDep {
          placeholder: url_dep.placeholder.clone(),
          resolved_path: resolved,
          fragment,
        });
      }
    }
  }

  Ok((result.code, resolved_deps))
}

// ---------------------------------------------------------------------------
// Asset emission
// ---------------------------------------------------------------------------

/// Emit all resolved url() assets via `ctx.emit_file()` and record the
/// placeholder → reference_id mapping in `UrlPlaceholders`.
pub fn emit_url_assets(
  ctx: &PluginContext,
  url_deps: &[ResolvedUrlDep],
  placeholders: &UrlPlaceholders,
) -> anyhow::Result<()> {
  for dep in url_deps {
    if placeholders.inner.contains_key(&dep.placeholder) {
      continue; // Already emitted (e.g. same asset referenced from multiple CSS files)
    }

    let asset_path = &dep.resolved_path;
    if !asset_path.exists() {
      // Asset doesn't exist — leave the placeholder as-is so it becomes a
      // broken reference rather than crashing the build. A warning could be
      // added here in the future.
      continue;
    }

    let source = std::fs::read(asset_path)
      .map_err(|e| anyhow::anyhow!("Failed to read asset {}: {e}", asset_path.display()))?;

    let file_name = asset_path
      .file_name()
      .map(|n| n.to_string_lossy().into_owned())
      .unwrap_or_else(|| "asset".to_owned());

    let reference_id = ctx.emit_file(
      EmittedAsset {
        name: Some(file_name),
        original_file_name: Some(asset_path.to_string_lossy().into_owned()),
        file_name: None,
        source: StrOrBytes::Bytes(source),
      },
      None,
      None,
    )?;

    placeholders.inner.insert(dep.placeholder.clone(), reference_id);
  }

  Ok(())
}

// ---------------------------------------------------------------------------
// Placeholder replacement
// ---------------------------------------------------------------------------

/// Replace all lightningcss url() placeholders in the CSS string with
/// output-relative paths to the emitted assets.
pub fn replace_url_placeholders(
  css: &str,
  placeholders: &UrlPlaceholders,
  url_deps: &[ResolvedUrlDep],
  ctx: &PluginContext,
  css_output_dir: &str,
) -> anyhow::Result<String> {
  let mut result = css.to_owned();

  for dep in url_deps {
    if let Some(reference_id) = placeholders.inner.get(&dep.placeholder) {
      let asset_filename = ctx.get_file_name(&reference_id)?;

      // Compute path relative from CSS output directory to the asset
      let relative_path = compute_relative_path(css_output_dir, &asset_filename);

      let mut final_url = relative_path;
      if let Some(frag) = &dep.fragment {
        final_url.push_str(frag);
      }

      result = result.replace(&dep.placeholder, &final_url);
    }
  }

  Ok(result)
}

/// Compute a relative path from `from_dir` to `to_path`.
/// Both paths are treated as output-relative (e.g. "assets/" and "assets/image.png").
fn compute_relative_path(from_dir: &str, to_path: &str) -> String {
  use sugar_path::SugarPath;

  let to = Path::new(to_path);
  let from = Path::new(from_dir);
  let relative = to.relative(from);
  let result = relative.to_slash_lossy().into_owned();

  if result.starts_with('.') { result } else { format!("./{result}") }
}

/// Initialize the `UrlPlaceholders` shared state in `PluginContextMeta`.
pub fn init_url_placeholders(ctx: &PluginContext) {
  ctx.meta().insert(Arc::new(UrlPlaceholders::default()));
}
