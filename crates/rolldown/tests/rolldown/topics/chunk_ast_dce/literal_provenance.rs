use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  Output, RawCompressOptions, RawMangleOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
  SourceMapIgnoreList,
};
use rolldown_utils::pattern_filter::StringOrRegex;

#[tokio::test(flavor = "multi_thread")]
async fn inlined_literal_sources() {
  for mode in ["compress", "mangle", "full"] {
    let output = Bundler::new(BundlerOptions {
      cwd: Some(
        concat!(
          env!("CARGO_MANIFEST_DIR"),
          "/tests/rolldown/topics/chunk_ast_dce/literal_provenance"
        )
        .into(),
      ),
      input: Some(vec![InputItem { name: Some("entry".into()), import: "./entry.js".into() }]),
      sourcemap: Some(SourceMapType::File),
      minify: Some(if mode == "full" {
        RawMinifyOptions::Bool(true)
      } else {
        RawMinifyOptions::Object(RawMinifyOptionsDetailed {
          compress: (mode == "compress").then(RawCompressOptions::default),
          mangle: (mode == "mangle").then(RawMangleOptions::default),
          mangle_properties: None,
          remove_whitespace: false,
          ascii_only: false,
        })
      }),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| match asset {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    // Assert the serialized map consumers receive, including concat VLQ state.
    let json = chunk.map.as_ref().unwrap().to_json_string();
    let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
    // Finalizers synthesize default-export binding names that need not occur in source text.
    // Validate serialized coordinates without requiring those names to spell source tokens.
    let mut previous = None;
    for token in map.get_source_view_tokens() {
      let generated = (token.get_dst_line(), token.get_dst_col());
      assert!(previous.is_none_or(|previous| previous <= generated));
      previous = Some(generated);
      let line = chunk.code.split('\n').nth(generated.0 as usize).unwrap();
      assert!(generated.1 as usize <= line.encode_utf16().count());
      if let Some(source) = token.get_source_id() {
        let content = map.get_source_content(source).unwrap();
        let line = content.split('\n').nth(token.get_src_line() as usize).unwrap();
        assert!(token.get_src_col() as usize <= line.encode_utf16().count());
      }
      if let Some(name) = token.get_name_id() {
        assert!(map.get_name(name).is_some());
      }
    }
    for (marker, source, content, original) in [
      (
        "shader-string-anchor",
        "string.js",
        include_str!("literal_provenance/string.js"),
        "'shader-string-anchor'",
      ),
      (
        "shader-template-anchor",
        "template.js",
        include_str!("literal_provenance/template.js"),
        "`shader-template-anchor`",
      ),
    ] {
      let generated = chunk.code.find(marker).unwrap() - 1;
      super::map_assertions::assert_mapping_at(
        &chunk.code,
        &map,
        generated,
        source,
        content,
        content.find(original).unwrap(),
      );
    }
    assert!(chunk.code.contains("shader-string-anchor"));
    assert!(chunk.code.contains("shader-template-anchor"));
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn eliminated_donor_sources_drive_ignore_list() {
  for (retained, cwd, input) in [
    (
      false,
      concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../rollup/test/sourcemaps/samples/ignore-list-source-files"
      ),
      "./main.js",
    ),
    (
      true,
      concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/rolldown/topics/chunk_ast_dce/literal_provenance"
      ),
      "./retained-entry.js",
    ),
  ] {
    let output = Bundler::new(BundlerOptions {
      cwd: Some(cwd.into()),
      input: Some(vec![InputItem { name: Some("entry".into()), import: input.into() }]),
      sourcemap: Some(SourceMapType::File),
      sourcemap_ignore_list: Some(SourceMapIgnoreList::from_string_or_regex(
        StringOrRegex::String("lib".into()),
      )),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| match asset {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    let json = chunk.map.as_ref().unwrap().to_json_string();
    let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
    super::map_assertions::assert_valid_map(&chunk.code, &map);
    let sources = map.get_sources().collect::<Vec<_>>();
    if retained {
      assert_eq!(sources.len(), 2, "{sources:?}");
      assert!(sources.iter().any(|source| source.ends_with("retained-lib.js")));
      assert!(sources.iter().any(|source| source.ends_with("retained-entry.js")));
      let expected = sources
        .iter()
        .enumerate()
        .filter(|(_, source)| source.contains("lib"))
        .map(|(index, _)| u32::try_from(index).unwrap())
        .collect::<Vec<_>>();
      assert_eq!(expected.len(), 1);
      assert!(expected[0] < u32::try_from(sources.len()).unwrap());
      assert_eq!(map.get_ignore_list(), Some(expected.as_slice()));
    } else {
      assert_eq!(sources.len(), 1, "{sources:?}");
      assert!(sources[0].ends_with("main.js"));
      assert!(map.get_ignore_list().is_none_or(<[u32]>::is_empty));
      assert!(map.get_source_view_tokens().all(|token| {
        token
          .get_source_id()
          .is_none_or(|source| map.get_source(source).unwrap().ends_with("main.js"))
      }));
      let content = include_str!(
        "../../../../../../rollup/test/sourcemaps/samples/ignore-list-source-files/main.js"
      );
      super::map_assertions::assert_mapping_at(
        &chunk.code,
        &map,
        chunk.code.find("3;").unwrap(),
        "main.js",
        content,
        content.find("one + two").unwrap(),
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn consumer_comments_survive_literal_inlining() {
  for sourcemap in [None, Some(SourceMapType::File)] {
    let output = Bundler::new(BundlerOptions {
      cwd: Some(
        concat!(
          env!("CARGO_MANIFEST_DIR"),
          "/tests/rolldown/topics/chunk_ast_dce/literal_provenance"
        )
        .into(),
      ),
      input: Some(vec![InputItem { name: Some("entry".into()), import: "./commented.js".into() }]),
      sourcemap,
      minify: Some(RawMinifyOptions::Bool(true)),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| match asset {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    assert_eq!(chunk.code.matches("@vite-ignore").count(), 2, "{}", chunk.code);
    assert!(chunk.code.contains("shader-string-anchor"), "{}", chunk.code);
  }
}
