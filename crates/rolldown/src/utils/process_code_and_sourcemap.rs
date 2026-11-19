use std::{borrow::Cow, path::Path};

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use rolldown_common::{NormalizedBundlerOptions, OutputAsset, SourceMapType};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_sourcemap::SourceMap;
use rolldown_std_utils::relative_path_to_slash;
use sugar_path::SugarPath;
use url::Url;

use super::uuid::uuid_v4_string_from_u128;

// URL delimiters must remain filename data.
const SOURCEMAP_FILENAME_ENCODE_SET: &AsciiSet = &CONTROLS
  .add(b' ')
  .add(b'"')
  .add(b'#')
  .add(b'%')
  .add(b':')
  .add(b'<')
  .add(b'>')
  .add(b'?')
  .add(b'\\')
  .add(b'`')
  .add(b'{')
  .add(b'}');

#[cfg(feature = "experimental")]
pub async fn process_code_and_sourcemap(
  options: &NormalizedBundlerOptions,
  code: &mut String,
  map: &mut SourceMap,
  file_dir: &Path,
  filename: &str,
  debug_id: u128,
  sourcemap_filename: Option<String>,
) -> BuildResult<Option<OutputAsset>> {
  prepare_sourcemap(options, map, file_dir, filename).await?;
  let map_filename = sourcemap_filename.unwrap_or_else(|| format!("{filename}.map"));
  emit_sourcemap(options, code, map, &map_filename, debug_id)
}

// See internal-docs/chunk-hash/implementation.md for source indices and callback handling.
pub async fn prepare_sourcemap(
  options: &NormalizedBundlerOptions,
  map: &mut SourceMap,
  file_dir: &Path,
  filename: &str,
) -> BuildResult<()> {
  let file_base_name = Path::new(filename).file_name().expect("should have file name");
  map.set_file(file_base_name.to_string_lossy().as_ref());

  if options.sourcemap_exclude_sources {
    map.set_source_contents(vec![]);
  }

  let map_filename = format!("{}.map", file_base_name.to_string_lossy());
  let map_path = file_dir.join(&map_filename);

  if let Some(source_map_ignore_list) = &options.sourcemap_ignore_list {
    let relative_sources = map
      .get_sources()
      .flatten()
      .map(|source| source.as_path().relative(file_dir).to_string_lossy().into_owned())
      .collect::<Vec<_>>();

    let ignored = match source_map_ignore_list {
      rolldown_common::SourceMapIgnoreList::Boolean(_)
      | rolldown_common::SourceMapIgnoreList::StringOrRegex(_) => {
        // Fast path: no async overhead for static values (boolean/string/regex)
        relative_sources
          .iter()
          .map(|source| source_map_ignore_list.exec_static(source))
          .collect::<Vec<_>>()
      }
      rolldown_common::SourceMapIgnoreList::Fn(_) => {
        // Slow path: one call decides every source, so the napi boundary is crossed once per
        // sourcemap instead of once per source.
        let source_count = relative_sources.len();
        let ignored = source_map_ignore_list
          .exec_dynamic(relative_sources, map_path.to_string_lossy().as_ref())
          .await?;
        if ignored.len() != source_count {
          return Err(
            anyhow::anyhow!(
              "`sourcemapIgnoreList` returned {} results for {} sources",
              ignored.len(),
              source_count
            )
            .into(),
          );
        }
        ignored
      }
    };

    #[expect(clippy::cast_possible_truncation)]
    let ignore_list = map
      .get_sources()
      .enumerate()
      .filter_map(|(index, source)| source.map(|_| index))
      .zip(ignored)
      .filter_map(|(index, should_ignore)| should_ignore.then_some(index as u32))
      .collect::<Vec<_>>();

    if !ignore_list.is_empty() {
      map.set_ignore_list(ignore_list);
    }
  }

  if let Some(sourcemap_path_transform) = &options.sourcemap_path_transform {
    let map_path = map_path.to_string_lossy();
    let relative_sources = map
      .get_sources()
      .flatten()
      .map(|source| source.as_path().relative(file_dir).to_string_lossy().into_owned())
      .collect::<Vec<_>>();
    let source_count = relative_sources.len();

    // One call rewrites every source, so the napi boundary is crossed once per sourcemap instead
    // of once per source.
    let sources = sourcemap_path_transform.call(relative_sources, map_path.as_ref()).await?;
    if sources.len() != source_count {
      return Err(
        anyhow::anyhow!(
          "`sourcemapPathTransform` returned {} results for {} sources",
          sources.len(),
          source_count
        )
        .into(),
      );
    }

    #[cfg(windows)]
    // Normalize the windows path.
    let sources = sources
      .into_iter()
      .map(|source| source.replace(std::path::MAIN_SEPARATOR, "/"))
      .collect::<Vec<_>>();

    if map.get_sources().all(|source| source.is_some()) {
      map.set_sources(sources.into_iter().map(Some));
    } else {
      let mut transformed = sources.into_iter();
      let sources = map
        .get_sources()
        .map(|source| source.map(|_| transformed.next().expect("source count was checked")))
        .collect::<Vec<_>>();
      map.set_sources(sources);
    }
  } else if cfg!(windows) {
    // Normalize the windows path at final.
    let sources = map
      .get_sources()
      .map(|x| x.map(|x| relative_path_to_slash(x.as_path(), file_dir)))
      .collect::<Vec<_>>();
    map.set_sources(sources);
  } else {
    map.set_sources(
      map
        .get_sources()
        .map(|x| x.map(|x| x.as_path().relative(file_dir).to_string_lossy().into_owned()))
        .collect::<Vec<_>>(),
    );
  }

  Ok(())
}

pub fn emit_sourcemap(
  options: &NormalizedBundlerOptions,
  code: &mut String,
  map: &mut SourceMap,
  map_filename: &str,
  debug_id: u128,
) -> BuildResult<Option<OutputAsset>> {
  if options.sourcemap_debug_ids && options.sourcemap.is_some() {
    let debug_id_str = uuid_v4_string_from_u128(debug_id);
    map.set_debug_id(&debug_id_str);

    process_sourcemap_related_reference(code, |source| {
      source.push_str("# debugId=");
      source.push_str(debug_id_str.as_str());
      Ok(())
    })?;
  }

  if let Some(sourcemap) = &options.sourcemap {
    match sourcemap {
      SourceMapType::File | SourceMapType::Hidden => {
        // `to_json_string` reserves six times the map's string bytes up front and returns the
        // buffer unshrunk. This string lives on as the `.map` asset until the output is written
        // (or longer, when the caller keeps the `BundleOutput`), so give the slack back now.
        let mut source = map.to_json_string();
        source.shrink_to_fit();
        if matches!(sourcemap, SourceMapType::File) {
          process_sourcemap_related_reference(code, |source| {
            source.push_str("# sourceMappingURL=");
            source.push_str(&sourcemap_file_url(options, map_filename)?);
            Ok(())
          })?;
        }
        return Ok(Some(OutputAsset {
          filename: map_filename.into(),
          source: source.into(),
          original_file_names: vec![],
          names: vec![],
        }));
      }
      SourceMapType::Inline => {
        let data_url = map.to_data_url();
        process_sourcemap_related_reference(code, |source| {
          source.push_str("# sourceMappingURL=");
          source.push_str(&data_url);
          Ok(())
        })?;
      }
    }
  }

  Ok(None)
}

pub fn sourcemap_file_url(
  options: &NormalizedBundlerOptions,
  map_filename: &str,
) -> BuildResult<String> {
  match &options.sourcemap_base_url {
    Some(url_string) => {
      let encoded_filename: Cow<'_, str> =
        utf8_percent_encode(map_filename, SOURCEMAP_FILENAME_ENCODE_SET).into();
      let url = Url::parse(url_string)
        .and_then(|base| base.join(&encoded_filename))
        .map_err_to_unhandleable()?;
      Ok(url.into())
    }
    None => {
      let filename = Path::new(map_filename)
        .file_name()
        .ok_or(anyhow::anyhow!("should have filename"))?
        .to_string_lossy();
      let encoded_filename: Cow<'_, str> =
        utf8_percent_encode(&filename, SOURCEMAP_FILENAME_ENCODE_SET).into();
      Ok(encoded_filename.into_owned())
    }
  }
}

fn process_sourcemap_related_reference(
  source: &mut String,
  mut reference_body_processor: impl FnMut(&mut String) -> BuildResult<()>,
) -> BuildResult<()> {
  source.push_str("\n//");
  reference_body_processor(source)
}

#[cfg(test)]
mod tests {
  use super::{emit_sourcemap, prepare_sourcemap};
  use rolldown_common::{
    NormalizedBundlerOptions, SourceMapIgnoreList, SourceMapPathTransform, SourceMapType,
  };
  use rolldown_sourcemap::{SourceMap, SourceMapBuilder};

  #[tokio::test]
  async fn null_sources_keep_their_indices_and_skip_path_callbacks() {
    let options = NormalizedBundlerOptions {
      sourcemap_ignore_list: Some(SourceMapIgnoreList::new(std::sync::Arc::new(|sources, _| {
        assert_eq!(sources, ["input.js"]);
        Box::pin(async { Ok(vec![true]) })
      }))),
      sourcemap_path_transform: Some(SourceMapPathTransform::new(std::sync::Arc::new(
        |sources, _| {
          assert_eq!(sources, ["input.js"]);
          Box::pin(async { Ok(vec!["renamed.js".into()]) })
        },
      ))),
      ..Default::default()
    };
    let dir = std::env::current_dir().unwrap();
    let input = dir.join("input.js").to_string_lossy().into_owned();
    let mut map = SourceMap::new(
      None,
      vec![],
      None,
      vec![None, Some(input.into())],
      vec![],
      Box::new([]),
      None,
    );
    prepare_sourcemap(&options, &mut map, &dir, "out.js").await.unwrap();
    assert_eq!(map.get_sources().collect::<Vec<_>>(), [None, Some("renamed.js")]);
    assert_eq!(map.get_ignore_list(), Some(&[1][..]));
  }

  #[tokio::test]
  async fn null_sources_survive_relative_path_normalization() {
    let dir = std::env::current_dir().unwrap();
    let input = dir.join("input.js").to_string_lossy().into_owned();
    let mut map = SourceMap::new(
      None,
      vec![],
      None,
      vec![None, Some(input.into())],
      vec![],
      Box::new([]),
      None,
    );
    prepare_sourcemap(&NormalizedBundlerOptions::default(), &mut map, &dir, "out.js")
      .await
      .unwrap();
    assert_eq!(map.get_sources().collect::<Vec<_>>(), [None, Some("input.js")]);
  }

  #[test]
  fn sourcemap_filenames_remain_in_comments() {
    let options =
      NormalizedBundlerOptions { sourcemap: Some(SourceMapType::File), ..Default::default() };
    let mut map = SourceMapBuilder::default().into_sourcemap().into_owned();
    let filename = "test\u{2028}(filename,{}).map";
    let mut code = String::from("export const value = 42;");
    let asset = emit_sourcemap(&options, &mut code, &mut map, filename, 0).unwrap().unwrap();
    assert_eq!(asset.filename.as_str(), filename);
    assert_eq!(
      code,
      "export const value = 42;\n\
       //# sourceMappingURL=test%E2%80%A8(filename,%7B%7D).map"
    );
  }

  #[test]
  fn sourcemap_urls_preserve_filename_data() {
    for base_url in [None, Some("https://example.test/base*/")] {
      for (filename, encoded_filename) in [
        ("space #?%*.map", "space%20%23%3F%25*.map"),
        ("line\n\r\u{2028}\u{2029}.map", "line%0A%0D%E2%80%A8%E2%80%A9.map"),
        // spellchecker:ignore-next-line
        ("café.map", "caf%C3%A9.map"),
      ] {
        let options = NormalizedBundlerOptions {
          sourcemap: Some(SourceMapType::File),
          sourcemap_base_url: base_url.map(String::from),
          ..Default::default()
        };
        let mut map = SourceMapBuilder::default().into_sourcemap().into_owned();
        let mut code = String::new();
        emit_sourcemap(&options, &mut code, &mut map, filename, 0).unwrap();
        let base_url = base_url.unwrap_or_default();
        assert_eq!(code, format!("\n//# sourceMappingURL={base_url}{encoded_filename}"));
      }
    }
  }
}
