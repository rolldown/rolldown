use oxc_sourcemap::SourcemapVisualizer;
use string_wizard::{MagicString, ReplaceOptions, SourceMapOptions};

// NOTE: The plain `replace` / `replace_all` unit tests were moved to
// `packages/rolldown/tests/magic-string/rolldown-magic-string.test.ts` so they
// exercise the JS binding's UTF-16 -> UTF-8 conversion. The sourcemap snapshot
// tests below stay here because they rely on `insta` + `SourcemapVisualizer`.

#[test]
fn replace_sourcemap_with_chinese() {
  let code = "测 2 测 4";
  let mut s = MagicString::new(code);

  s.replace_with(
    "测",
    "试",
    ReplaceOptions { count: usize::MAX, store_original_in_sourcemap: true },
  )
  .unwrap();

  let output = s.to_string();
  let sourcemap = s.source_map(SourceMapOptions {
    source: code.to_string().into(),
    include_content: true,
    ..Default::default()
  });
  println!("sourcemap {sourcemap:#?}");
  let visualizer = SourcemapVisualizer::new(&output, &sourcemap);
  assert_eq!(s.to_string(), "试 2 试 4");
  insta::assert_snapshot!("output", output);
  insta::assert_snapshot!("sourcemap_with_chinese", visualizer.get_text());
}

#[test]
fn replace_sourcemap() {
  let code = "* 2 * 4";
  let mut s = MagicString::new(code);

  s.replace_with("*", "#", ReplaceOptions { count: usize::MAX, store_original_in_sourcemap: true })
    .unwrap();

  let output = s.to_string();
  let sourcemap = s.source_map(SourceMapOptions {
    source: code.to_string().into(),
    include_content: true,
    ..Default::default()
  });
  println!("sourcemap {sourcemap:#?}");
  let visualizer = SourcemapVisualizer::new(&output, &sourcemap);
  assert_eq!(s.to_string(), "# 2 # 4");
  insta::assert_snapshot!("sourcemap", visualizer.get_text());
}
