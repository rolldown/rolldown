use std::{borrow::Cow, path::Path, sync::Arc};

use arcstr::ArcStr;
use memchr::memmem;
use rolldown_common::{EmittedAsset, ModuleType, ResolvedExternal, StrOrBytes};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookResolveIdArgs,
  HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext, PluginHookMeta,
  PluginOrder,
};
use rolldown_utils::url::clean_url;
use rustc_hash::FxHashSet;
use string_wizard::{MagicString, SourceMapOptions};
use sugar_path::SugarPath;

const PREFIX: &str = "__ROLLDOWN_COPY_MODULE__#";

#[derive(Debug)]
pub struct CopyModulePlugin {
  copy_extensions: FxHashSet<String>,
}

impl CopyModulePlugin {
  pub fn new(module_types: &rustc_hash::FxHashMap<Cow<'static, str>, ModuleType>) -> Self {
    let mut copy_extensions = FxHashSet::default();
    for (ext, module_type) in module_types {
      if matches!(module_type, ModuleType::Copy) {
        let ext = ext.strip_prefix('.').unwrap_or(ext);
        copy_extensions.insert(ext.to_string());
      }
    }
    Self { copy_extensions }
  }

  pub fn is_active(&self) -> bool {
    !self.copy_extensions.is_empty()
  }
}

impl Plugin for CopyModulePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:copy-module")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::RenderChunk
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    // Run before users' resolve_id hooks to ensure:
    // - For matched modules, to handle it correctly without users' interference.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if self.copy_extensions.is_empty() {
      return Ok(None);
    }

    // Don't re-resolve our own prefixed IDs
    if args.specifier.starts_with(PREFIX) {
      return Ok(None);
    }

    // Resolve the specifier to get the absolute path
    let resolved = ctx.resolve(args.specifier, args.importer, None).await?;

    let resolved_id = match resolved {
      Ok(id) => id,
      Err(_) => return Ok(None),
    };

    // Strip query/fragment (e.g. `file.txt?url`) before extension check and file read
    let clean_id = clean_url(resolved_id.id.as_str());
    let resolved_path = Path::new(clean_id);

    // Check if the resolved path has a copy extension
    let ext = match resolved_path.extension().and_then(|e| e.to_str()) {
      Some(e) => e,
      None => return Ok(None),
    };

    if !self.copy_extensions.contains(ext) {
      return Ok(None);
    }

    // Read the file bytes asynchronously to avoid blocking the tokio worker thread
    let bytes = tokio::fs::read(clean_id)
      .await
      .map_err(|e| anyhow::anyhow!("Failed to read copy module {}: {e}", resolved_id.id))?;

    // Derive a name from the file path
    let file_name =
      resolved_path.file_name().and_then(|n| n.to_str()).unwrap_or("asset").to_string();

    // Use relative path for original_file_name to avoid leaking absolute paths into output
    let original_file_name =
      resolved_path.strip_prefix(ctx.cwd()).unwrap_or(resolved_path).to_string_lossy().into_owned();

    // Emit the file as an asset
    let reference_id = ctx
      .emit_file_async(EmittedAsset {
        name: Some(file_name),
        original_file_name: Some(original_file_name),
        source: StrOrBytes::Bytes(bytes),
        ..Default::default()
      })
      .await?;

    // Add watch file for watch mode (use the clean absolute path)
    ctx.add_watch_file(clean_id);

    // Return a prefixed external ID — the prefix will be rewritten in render_chunk
    let placeholder_id: ArcStr = format!("{PREFIX}{reference_id}").into();

    Ok(Some(HookResolveIdOutput {
      id: placeholder_id,
      external: Some(ResolvedExternal::Bool(true)),
      ..Default::default()
    }))
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    // Run before users' render_chunk hooks to ensure:
    // - The placeholder IDs are replaced before any user hooks, so they won't see the placeholder IDs and won't interfere with our processing.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn render_chunk(
    &self,
    ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    // Quick bail: if the code doesn't contain our prefix, nothing to do
    if !args.code.contains(PREFIX) {
      return Ok(None);
    }

    let chunk_filename = &args.chunk.filename;
    let code = &args.code;
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
        map: args.options.sourcemap.is_some().then(|| {
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
