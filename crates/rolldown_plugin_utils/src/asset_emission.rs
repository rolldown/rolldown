use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use memchr::memmem;
use rolldown_common::{EmittedAsset, StrOrBytes};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookTransformOutputMap, PluginContext,
};
use rolldown_std_utils::relative_path_as_js_specifier;
use string_wizard::{MagicString, SourceMapOptions};

pub async fn emit_asset(
  ctx: &PluginContext,
  clean_id: &str,
  read_error: impl FnOnce(std::io::Error) -> anyhow::Error,
) -> anyhow::Result<ArcStr> {
  let path = Path::new(clean_id);
  let bytes = tokio::fs::read(clean_id).await.map_err(read_error)?;
  let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("asset").to_string();
  let original_file_name =
    path.strip_prefix(ctx.cwd()).unwrap_or(path).to_string_lossy().into_owned();

  let reference_id = ctx
    .emit_file_async(EmittedAsset {
      name: Some(file_name),
      original_file_name: Some(original_file_name),
      source: StrOrBytes::Bytes(bytes),
      ..Default::default()
    })
    .await?;

  Ok(reference_id)
}

pub fn rewrite_emitted_asset_references(
  ctx: &PluginContext,
  args: &HookRenderChunkArgs<'_>,
  prefix: &str,
) -> Option<HookRenderChunkOutput> {
  if !args.code.contains(prefix) {
    return None;
  }

  let chunk_filename = &args.chunk.filename;
  let code = args.code.as_str();
  let mut magic_string = MagicString::new(code);
  let mut changed = false;
  let finder = memmem::find_iter(code.as_bytes(), prefix.as_bytes());

  for abs_pos in finder {
    let after_prefix = abs_pos + prefix.len();
    let rest = &code[after_prefix..];
    let ref_end = rest.find(['"', '\'']).unwrap_or(rest.len());
    let ref_id = &rest[..ref_end];

    if ref_id.is_empty() {
      continue;
    }

    let Ok(asset_filename) = ctx.get_file_name(ref_id) else {
      continue;
    };
    let relative = compute_relative_path(chunk_filename, &asset_filename);
    let end = after_prefix + ref_end;

    #[expect(clippy::cast_possible_truncation)]
    if magic_string.update(abs_pos as u32, end as u32, relative).is_ok() {
      changed = true;
    }
  }

  if changed {
    Some(HookRenderChunkOutput {
      code: magic_string.to_string(),
      map: HookTransformOutputMap::from_if_enabled(args.options.sourcemap.is_some(), || {
        magic_string.source_map(SourceMapOptions {
          hires: string_wizard::Hires::Boundary,
          include_content: false,
          source: Arc::from(args.chunk.filename.as_str()),
        })
      }),
    })
  } else {
    None
  }
}

fn compute_relative_path(chunk_filename: &str, asset_filename: &str) -> String {
  let chunk_dir = Path::new(chunk_filename).parent().unwrap_or(Path::new(""));

  relative_path_as_js_specifier(asset_filename, chunk_dir)
}
