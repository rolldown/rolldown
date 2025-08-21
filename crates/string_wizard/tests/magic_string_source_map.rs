use oxc_sourcemap::SourcemapVisualizer;
use string_wizard::{Hires, MagicString, ReplaceOptions, SourceMapOptions, UpdateOptions};

#[test]
fn basic() {
  let input = "<div>\n  hello, world\n</div>";
  let mut s = MagicString::new(input);
  let update_options = UpdateOptions { keep_original: true, ..Default::default() };
  s.update_with(1, 2, "v", update_options.clone())
    .update_with(3, 4, "d", update_options.clone())
    .update_with(input.len() - 4, input.len() - 1, "h1", update_options.clone());

  let sm = s.source_map(SourceMapOptions { include_content: true, ..Default::default() });
  insta::assert_snapshot!("basic1", sm.to_json_string());

  s.prepend("import React from 'react';\n");
  let sm = s.source_map(SourceMapOptions { include_content: true, ..Default::default() });
  insta::assert_snapshot!("basic2", sm.to_json_string());

  let sm = s.source_map(SourceMapOptions {
    include_content: true,
    hires: Hires::True,
    ..Default::default()
  });
  insta::assert_snapshot!("basic3", sm.to_json_string());
}

#[test]
fn test_hires() {
  let code = r#"
function test() {
  console.log("foo")
  console.error("bar")
}
"#;
  let mut s = MagicString::new(code);

  s.replace_with("foo", "hello", ReplaceOptions::default());
  s.replace_with("bar", "world", ReplaceOptions::default());
  let output = s.to_string();
  assert_eq!(
    s.to_string(),
    r#"
function test() {
  console.log("hello")
  console.error("world")
}
"#
  );

  fn visualize(s: &MagicString, hires: Hires, output: &str) -> String {
    let sourcemap = s.source_map(SourceMapOptions { hires, ..Default::default() });
    let visualizer = SourcemapVisualizer::new(output, &sourcemap);
    visualizer.get_text()
  }

  insta::assert_snapshot!("hires_false", visualize(&s, Hires::False, &output));
  insta::assert_snapshot!("hires_true", visualize(&s, Hires::True, &output));
  insta::assert_snapshot!("hires_boundary", visualize(&s, Hires::Boundary, &output));
}
