use std::path::Path;

use oxc::ast::CommentKind;
use rolldown_common::{NormalizedBundlerOptions, OutputAsset, SourceMapType};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_sourcemap::SourceMap;
use sugar_path::SugarPath;
use url::Url;

use super::uuid::uuid_v4_string_from_u128;

pub async fn process_code_and_sourcemap(
  options: &NormalizedBundlerOptions,
  code: &mut String,
  map: &mut SourceMap,
  file_dir: &Path,
  filename: &str,
  debug_id: u128,
  is_css: bool,
) -> BuildResult<Option<OutputAsset>> {
  let source_map_link_comment_kind = if is_css { CommentKind::Block } else { CommentKind::Line };
  let file_base_name = Path::new(filename).file_name().expect("should have file name");
  map.set_file(file_base_name.to_string_lossy().as_ref());

  let map_filename = format!("{filename}.map");
  let map_path = file_dir.join(&map_filename);

  let paths =
    map.get_sources().map(|source| source.as_path().relative(file_dir)).collect::<Vec<_>>();
  // Here not normalize the windows path, the rollup `sourcemap_path_transform` ctx.options need to original path.
  let sources = paths.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>();
  map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());

  if let Some(source_map_ignore_list) = &options.sourcemap_ignore_list {
    let mut x_google_ignore_list = vec![];
    for (index, source) in map.get_sources().enumerate() {
      let should_ignore = match source_map_ignore_list {
        rolldown_common::SourceMapIgnoreList::Boolean(_)
        | rolldown_common::SourceMapIgnoreList::StringOrRegex(_) => {
          // Fast path: no async overhead for static values (boolean/string/regex)
          source_map_ignore_list.exec_static(source)
        }
        rolldown_common::SourceMapIgnoreList::Fn(_) => {
          // Slow path: async function call only when needed
          source_map_ignore_list.exec_dynamic(source, map_path.to_string_lossy().as_ref()).await?
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
    let mut sources = Vec::with_capacity(map.get_sources().count());
    for source in map.get_sources() {
      sources
        .push(sourcemap_path_transform.call(source, map_path.to_string_lossy().as_ref()).await?);
    }
    map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());
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

  // Normalize the windows path at final.
  let sources = map.get_sources().map(|x| x.to_slash_lossy().to_string()).collect::<Vec<_>>();
  map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());

  if let Some(sourcemap) = &options.sourcemap {
    match sourcemap {
      SourceMapType::File | SourceMapType::Hidden => {
        let source = map.to_json_string();
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
                  source.push_str(
                    &Path::new(&map_filename)
                      .file_name()
                      .ok_or(anyhow::anyhow!("should have filename"))?
                      .to_string_lossy(),
                  );
                }
              }
              Ok(())
            },
            source_map_link_comment_kind,
          )?;
        }
        return Ok(Some(OutputAsset {
          filename: map_filename.as_str().into(),
          source: source.into(),
          original_file_names: vec![],
          names: vec![],
        }));
      }
      SourceMapType::Inline => {
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
      }
    }
  }

  Ok(None)
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
    CommentKind::Block => {
      source.push_str("/*");
      reference_body_processor(source)?;
      source.push_str("*/");
    }
  }
  Ok(())
}
