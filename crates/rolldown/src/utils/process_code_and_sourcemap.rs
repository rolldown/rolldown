use std::path::Path;

use futures::future::try_join_all;
use oxc::ast::CommentKind;
use rolldown_common::{NormalizedBundlerOptions, OutputAsset, SourceMapType};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::{
  hash_placeholder::{
    HASH_PLACEHOLDER_LEFT_FINDER, extract_hash_placeholders, replace_placeholder_with_hash,
  },
  xxhash::encode_hash_with_base,
};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;
use url::Url;
use xxhash_rust::xxh3::Xxh3;

use super::uuid::uuid_v4_string_from_u128;

/// Returns `(sourcemap_filename, sourcemap_asset)`:
/// - `sourcemap_filename` is the final name reported on the output chunk. Any `[hash]`
///   placeholder left in `sourcemap_filename` (from `output.sourcemapFileNames`) is resolved
///   here from the exact serialized map being emitted, so the name is a faithful cache key
///   for the file's final contents (after `sourcemapPathTransform`, `sourcemapIgnoreList`,
///   `sourcemapExcludeSources`, debug ids, ...).
/// - `sourcemap_asset` is the `.map` file to emit (`sourcemap: true | 'hidden'` only).
#[expect(clippy::too_many_arguments)]
pub async fn process_code_and_sourcemap(
  options: &NormalizedBundlerOptions,
  code: &mut String,
  map: &mut SourceMap,
  file_dir: &Path,
  filename: &str,
  debug_id: u128,
  is_css: bool,
  sourcemap_filename: Option<String>,
) -> BuildResult<(Option<String>, Option<OutputAsset>)> {
  let source_map_link_comment_kind =
    if is_css { CommentKind::SingleLineBlock } else { CommentKind::Line };
  let file_base_name = Path::new(filename).file_name().expect("should have file name");
  map.set_file(file_base_name.to_string_lossy().as_ref());

  if options.sourcemap_exclude_sources {
    map.set_source_contents(vec![]);
  }

  let has_custom_map_filename = sourcemap_filename.is_some();
  let mut map_filename = sourcemap_filename.unwrap_or_else(|| format!("{filename}.map"));
  // Rollup always hands `sourcemapIgnoreList`/`sourcemapPathTransform` the default
  // `<chunk>.map` path, independent of `sourcemapFileNames` — which may still contain an
  // unresolved `[hash]` here, since that hash derives from the transformed map content.
  let map_path = file_dir.join(format!("{filename}.map"));

  if let Some(source_map_ignore_list) = &options.sourcemap_ignore_list {
    let mut x_google_ignore_list = vec![];
    for (index, source) in map.get_sources().enumerate() {
      let source = source.as_path().relative(file_dir);
      let should_ignore = match source_map_ignore_list {
        rolldown_common::SourceMapIgnoreList::Boolean(_)
        | rolldown_common::SourceMapIgnoreList::StringOrRegex(_) => {
          // Fast path: no async overhead for static values (boolean/string/regex)
          source_map_ignore_list.exec_static(source.to_string_lossy().as_ref())
        }
        rolldown_common::SourceMapIgnoreList::Fn(_) => {
          // Slow path: async function call only when needed
          source_map_ignore_list
            .exec_dynamic(source.to_string_lossy().as_ref(), map_path.to_string_lossy().as_ref())
            .await?
        }
      };

      if should_ignore {
        #[expect(clippy::cast_possible_truncation)]
        x_google_ignore_list.push(index as u32);
      }
    }
    if !x_google_ignore_list.is_empty() {
      map.set_x_google_ignore_list(x_google_ignore_list);
    }
  }

  if let Some(sourcemap_path_transform) = &options.sourcemap_path_transform {
    let map_path = map_path.to_string_lossy();
    let sources = try_join_all(map.get_sources().map(async |source| {
      let source = source.as_path().relative(file_dir);
      let source =
        sourcemap_path_transform.call(source.to_string_lossy().as_ref(), map_path.as_ref()).await?;
      #[cfg(windows)]
      {
        // Normalize the windows path.
        Ok::<_, anyhow::Error>(source.replace(std::path::MAIN_SEPARATOR, "/"))
      }
      #[cfg(not(windows))]
      {
        Ok::<_, anyhow::Error>(source)
      }
    }))
    .await?;

    map.set_sources(sources);
  } else if cfg!(windows) {
    // Normalize the windows path at final.
    let sources = map
      .get_sources()
      .map(|x| x.as_path().relative(file_dir).to_slash_lossy().to_string())
      .collect::<Vec<_>>();
    map.set_sources(sources);
  } else {
    map.set_sources(
      map
        .get_sources()
        .map(|x| x.as_path().relative(file_dir).to_string_lossy().into_owned())
        .collect::<Vec<_>>(),
    );
  }

  if options.sourcemap_debug_ids && options.sourcemap.is_some() {
    let debug_id_str = uuid_v4_string_from_u128(debug_id);
    map.set_debug_id(&debug_id_str);

    process_sourcemap_related_reference(
      code,
      |source| {
        source.push_str("# debugId=");
        source.push_str(debug_id_str.as_str());
        Ok(())
      },
      source_map_link_comment_kind,
    )?;
  }

  if let Some(sourcemap) = &options.sourcemap {
    match sourcemap {
      SourceMapType::File | SourceMapType::Hidden => {
        let source = map.to_json_string();
        resolve_sourcemap_hash_placeholders(
          &mut map_filename,
          &source,
          options.hash_characters.base(),
        );
        if matches!(sourcemap, SourceMapType::File) {
          process_sourcemap_related_reference(
            code,
            |source| {
              source.push_str("# sourceMappingURL=");

              match &options.sourcemap_base_url {
                Some(url_string) => {
                  let url = Url::parse(url_string)
                    .and_then(|base| base.join(&map_filename))
                    .map_err_to_unhandleable()?;
                  source.push_str(url.as_str());
                }
                None => {
                  // Emit a URL that resolves from the chunk's location to the map file: the
                  // plain basename for default sibling maps, a relative path when
                  // `sourcemapFileNames` places maps in another directory. (Rollup emits the
                  // bare basename even then, which cannot resolve — a known upstream quirk.)
                  let chunk_dir = Path::new(filename).parent().unwrap_or_else(|| Path::new(""));
                  source.push_str(&Path::new(&map_filename).relative(chunk_dir).to_slash_lossy());
                }
              }
              Ok(())
            },
            source_map_link_comment_kind,
          )?;
        }
        return Ok((
          Some(map_filename.clone()),
          Some(OutputAsset {
            filename: map_filename.as_str().into(),
            source: source.into(),
            original_file_names: vec![],
            names: vec![],
          }),
        ));
      }
      SourceMapType::Inline => {
        if has_custom_map_filename {
          // No `.map` file is written, but the resolved name is still reported on the
          // output chunk (Rollup parity).
          resolve_sourcemap_hash_placeholders(
            &mut map_filename,
            &map.to_json_string(),
            options.hash_characters.base(),
          );
        }
        let data_url = map.to_data_url();
        process_sourcemap_related_reference(
          code,
          |source| {
            source.push_str("# sourceMappingURL=");
            source.push_str(&data_url);
            Ok(())
          },
          source_map_link_comment_kind,
        )?;
        return Ok((has_custom_map_filename.then_some(map_filename), None));
      }
    }
  }

  Ok((None, None))
}

fn resolve_sourcemap_hash_placeholders(map_filename: &mut String, map_json: &str, hash_base: u8) {
  let placeholders = extract_hash_placeholders(map_filename, &HASH_PLACEHOLDER_LEFT_FINDER);
  if placeholders.is_empty() {
    return;
  }
  let mut hasher = Xxh3::default();
  hasher.update(map_json.as_bytes());
  let hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), hash_base);
  let hashes_by_placeholder = placeholders
    .iter()
    .map(|placeholder| ((*placeholder).to_string(), &hash[..placeholder.len()]))
    .collect::<FxHashMap<_, _>>();
  let resolved = replace_placeholder_with_hash(
    map_filename,
    &hashes_by_placeholder,
    &HASH_PLACEHOLDER_LEFT_FINDER,
  )
  .into_owned();
  *map_filename = resolved;
}

fn process_sourcemap_related_reference(
  source: &mut String,
  mut reference_body_processor: impl FnMut(&mut String) -> BuildResult<()>,
  comment_kind: CommentKind,
) -> BuildResult<()> {
  source.push('\n');
  match comment_kind {
    CommentKind::Line => {
      source.push_str("//");
      reference_body_processor(source)?;
    }
    CommentKind::SingleLineBlock | CommentKind::MultiLineBlock => {
      source.push_str("/*");
      reference_body_processor(source)?;
      source.push_str("*/");
    }
  }
  Ok(())
}
