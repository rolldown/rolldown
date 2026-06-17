use std::{borrow::Cow, path::Path, sync::Arc};

use memchr::memmem;
use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookRenderChunkArgs, HookRenderChunkOutput,
  HookRenderChunkReturn, HookTransformOutputMap, HookUsage, Plugin, PluginHookMeta, PluginOrder,
  SharedLoadPluginContext,
};
use rolldown_utils::url::clean_url;
use rustc_hash::FxHashSet;
use string_wizard::{MagicString, SourceMapOptions};
use sugar_path::SugarPath;

const PREFIX: &str = "__ROLLDOWN_ASSET__#";

#[derive(Debug)]
pub struct AssetModulePlugin {
  asset_extensions: FxHashSet<String>,
}

impl AssetModulePlugin {
  pub fn new(module_types: &rustc_hash::FxHashMap<Cow<'static, str>, ModuleType>) -> Self {
    let mut asset_extensions = FxHashSet::default();
    for (ext, module_type) in module_types {
      if matches!(module_type, ModuleType::Asset) {
        let ext = ext.strip_prefix('.').unwrap_or(ext);
        asset_extensions.insert(ext.to_string());
      }
    }
    Self { asset_extensions }
  }
}

impl Plugin for AssetModulePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset-module")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load | HookUsage::RenderChunk
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    // Run after user plugins so they can override asset loading
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }

  fn load(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    self.load_impl(ctx, args)
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    // Run before user plugins so placeholders are resolved first
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    // Quick bail: if the code doesn't contain our prefix, nothing to do
    if !args.code.contains(PREFIX) {
      return Ok(None);
    }

    let chunk_filename = &args.chunk.filename;
    let code = args.code.as_str();
    let mut magic_string = MagicString::new(code);
    let mut changed = false;

    // Use memchr for SIMD-accelerated substring search
    let finder = memmem::find_iter(code.as_bytes(), PREFIX.as_bytes());

    for abs_pos in finder {
      let after_prefix = abs_pos + PREFIX.len();

      // Extract ref_id: scan until we hit a quote (", ') or end of string
      let rest = &code[after_prefix..];
      let ref_end = rest.find(['"', '\'']).unwrap_or(rest.len());
      let ref_id = &rest[..ref_end];

      if ref_id.is_empty() {
        continue;
      }

      // Resolve the asset filename
      let asset_filename = match ctx.get_file_name(ref_id) {
        Ok(name) => name,
        Err(_) => continue,
      };

      // Compute relative path from chunk to asset
      let relative = compute_relative_path(chunk_filename, &asset_filename);

      let end = after_prefix + ref_end;
      #[expect(clippy::cast_possible_truncation)]
      if magic_string.update(abs_pos as u32, end as u32, relative).is_ok() {
        changed = true;
      }
    }

    if changed {
      Ok(Some(HookRenderChunkOutput {
        code: magic_string.to_string(),
        map: HookTransformOutputMap::from_if_enabled(args.options.sourcemap.is_some(), || {
          magic_string.source_map(SourceMapOptions {
            hires: string_wizard::Hires::Boundary,
            include_content: false,
            source: Arc::from(args.chunk.filename.as_str()),
          })
        }),
      }))
    } else {
      Ok(None)
    }
  }
}

impl AssetModulePlugin {
  async fn load_impl(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    // Determine if this module should be treated as an asset:
    // 1. Via asserted_module_type (e.g. from `new URL('./file', import.meta.url)`)
    // 2. Via file extension matching `moduleTypes` config
    //
    // Strip query/fragment (e.g. `file.png?url`) before extension check and path operations,
    // consistent with CopyModulePlugin's handling.
    let clean_id = clean_url(args.id);
    let is_asset = args.asserted_module_type.is_some_and(|ty| matches!(ty, ModuleType::Asset))
      || self.is_asset_by_extension(clean_id);

    if !is_asset {
      return Ok(None);
    }

    let path = Path::new(clean_id);

    // Read file as binary (use cleaned path for filesystem access)
    let bytes = tokio::fs::read(clean_id)
      .await
      .map_err(|e| anyhow::anyhow!("Failed to read asset module {clean_id}: {e}"))?;

    // Derive name from the cleaned file path
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("asset").to_string();

    // Use relative path for original_file_name to avoid leaking absolute paths
    let original_file_name =
      path.strip_prefix(ctx.cwd()).unwrap_or(path).to_string_lossy().into_owned();

    // Emit the file as an asset
    let reference_id = ctx
      .emit_file_async(EmittedAsset {
        name: Some(file_name),
        original_file_name: Some(original_file_name),
        source: StrOrBytes::Bytes(bytes),
        ..Default::default()
      })
      .await?;

    // Associate this module with the emitted file so the `new URL()` finalizer
    // can look up the asset filename by module ID (use original args.id for mapping)
    ctx.associate_module_with_file_ref(args.id, &reference_id);

    // Add watch file for watch mode (use cleaned path)
    ctx.add_watch_file(clean_id);

    // Return JS code that exports the asset placeholder via CJS.
    // Using `module.exports` ensures `require()` returns the string directly.
    // The placeholder will be resolved in renderChunk to the actual filename.
    let code = format!("module.exports = \"{PREFIX}{reference_id}\"");

    Ok(Some(HookLoadOutput {
      code: code.into(),
      module_type: Some(ModuleType::Js),
      // Mark as side-effect-free so tree-shaking excludes the module's statements
      // when nothing imports from it (e.g. `new URL()` only references).
      side_effects: Some(HookSideEffects::False),
      ..Default::default()
    }))
  }

  fn is_asset_by_extension(&self, id: &str) -> bool {
    if self.asset_extensions.is_empty() {
      return false;
    }
    Path::new(id)
      .extension()
      .and_then(|e| e.to_str())
      .is_some_and(|ext| self.asset_extensions.contains(ext))
  }
}

/// Compute the relative path from a chunk file to an asset file,
/// ensuring it starts with "./" for relative paths.
fn compute_relative_path(chunk_filename: &str, asset_filename: &str) -> String {
  let chunk_dir = Path::new(chunk_filename).parent().unwrap_or(Path::new(""));

  let relative = Path::new(asset_filename).relative(chunk_dir);
  let relative_str = relative.to_slash_lossy();

  if relative_str.starts_with("..") {
    relative_str.into_owned()
  } else if relative_str.is_empty() {
    ".".to_string()
  } else {
    format!("./{relative_str}")
  }
}

/// Resolve `__ROLLDOWN_ASSET__#<refId>` placeholders to chunk-relative asset
/// paths for code that does NOT pass through the `renderChunk` hook — namely HMR
/// and lazy-compilation patches. The normal generate path resolves these in
/// [`AssetModulePlugin::render_chunk`]; the HMR/lazy codegen assembles its output
/// directly and bypasses generate, so without this the placeholder would leak to
/// the browser and 404 (rolldown#9812, vitejs/vite#22596). See
/// `meta/design/plugin-asset-module.md`.
///
/// `chunk_filename` is the served filename of the patch (e.g. `hmr_patch_0.js`),
/// used to compute the path from the patch to each asset. `get_file_name`
/// resolves a reference id to its emitted filename (e.g. via
/// `FileEmitter::get_file_name`); a reference it cannot resolve is left as-is.
/// Returns the rewritten code, or `None` when there is nothing to resolve so
/// callers can skip the allocation.
pub fn resolve_asset_placeholders<S>(
  code: &str,
  chunk_filename: &str,
  get_file_name: impl Fn(&str) -> Option<S>,
) -> Option<String>
where
  S: AsRef<str>,
{
  // Quick bail mirrors `render_chunk`: most patches reference no assets.
  if !code.contains(PREFIX) {
    return None;
  }

  let mut out = String::with_capacity(code.len());
  let mut last = 0usize;
  let mut changed = false;

  for abs_pos in memmem::find_iter(code.as_bytes(), PREFIX.as_bytes()) {
    let after_prefix = abs_pos + PREFIX.len();
    // The ref id runs until the closing quote of the placeholder string literal.
    let rest = &code[after_prefix..];
    let ref_end = rest.find(['"', '\'']).unwrap_or(rest.len());
    let ref_id = &rest[..ref_end];
    if ref_id.is_empty() {
      continue;
    }
    let Some(asset_filename) = get_file_name(ref_id) else {
      // Unknown reference: leave the placeholder verbatim (it stays within the
      // verbatim ranges copied below, since `last` is not advanced past it).
      continue;
    };
    let relative = compute_relative_path(chunk_filename, asset_filename.as_ref());
    out.push_str(&code[last..abs_pos]);
    out.push_str(&relative);
    last = after_prefix + ref_end;
    changed = true;
  }

  if !changed {
    return None;
  }
  out.push_str(&code[last..]);
  Some(out)
}

#[cfg(test)]
mod tests {
  use super::resolve_asset_placeholders;

  #[test]
  fn returns_none_when_no_placeholder() {
    let out =
      resolve_asset_placeholders("export const a = 1;", "hmr_patch_0.js", |_| None::<String>);
    assert_eq!(out, None);
  }

  #[test]
  fn resolves_single_placeholder_relative_to_patch() {
    let code = r#"module.exports = "__ROLLDOWN_ASSET__#abc""#;
    let out = resolve_asset_placeholders(code, "hmr_patch_0.js", |id| {
      (id == "abc").then(|| "assets/img-h.png".to_string())
    });
    assert_eq!(out.as_deref(), Some(r#"module.exports = "./assets/img-h.png""#));
  }

  #[test]
  fn leaves_unknown_reference_verbatim() {
    let code = r#"f("__ROLLDOWN_ASSET__#missing")"#;
    // The only reference is unresolvable, so nothing changes.
    let out = resolve_asset_placeholders(code, "hmr_patch_0.js", |_| None::<String>);
    assert_eq!(out, None);
  }

  #[test]
  fn resolves_known_and_keeps_unknown() {
    let code = r#"["__ROLLDOWN_ASSET__#a","__ROLLDOWN_ASSET__#b"]"#;
    let out = resolve_asset_placeholders(code, "hmr_patch_0.js", |id| {
      (id == "a").then(|| "assets/a.png".to_string())
    });
    assert_eq!(out.as_deref(), Some(r#"["./assets/a.png","__ROLLDOWN_ASSET__#b"]"#));
  }
}
