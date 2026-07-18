use oxc_sourcemap::SourcemapVisualizer;
use string_wizard::{Hires, MagicString, ReplaceOptions, SourceMapOptions, UpdateOptions};

#[test]
fn basic() {
  let input = "<div>\n  hello, world\n</div>";
  let mut s = MagicString::new(input);
  let update_options = UpdateOptions { keep_original: true, ..Default::default() };
  s.update_with(1, 2, "v", update_options.clone())
    .unwrap()
    .update_with(3, 4, "d", update_options.clone())
    .unwrap()
    .update_with((input.len() - 4) as u32, (input.len() - 1) as u32, "h1", update_options)
    .unwrap();

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

  s.replace_with("foo", "hello", ReplaceOptions::default()).unwrap();
  s.replace_with("bar", "world", ReplaceOptions::default()).unwrap();
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

#[test]
fn sourcemap_locations_force_segments_at_low_resolution() {
  // Mirrors magic-string's "should generate a sourcemap using specified locations" test:
  // marked characters get their own mapping segment even with hires off. Locations are byte
  // offsets here (UTF-16 and byte offsets coincide on ASCII); the expected mappings string is
  // exactly what magic-string@0.30.21 asserts for the same input.
  let mut s = MagicString::new("abcdefghijkl");
  s.add_sourcemap_location(0);
  s.add_sourcemap_location(3);
  s.add_sourcemap_location(10);
  s.remove(6, 9).unwrap();
  let sm = s.source_map(SourceMapOptions { include_content: true, ..Default::default() });
  assert_eq!(sm.to_json().mappings, "AAAA,GAAG,GAAM,CAAC");
}

#[test]
fn sourcemap_location_inside_edited_chunk_has_no_effect() {
  // magic-string consults sourcemapLocations only while walking unedited chunks; a location
  // covered by an edit (here: a removal) emits nothing extra.
  let mut s = MagicString::new("abcdefghijkl");
  s.add_sourcemap_location(7);
  s.remove(6, 9).unwrap();
  let sm = s.source_map(SourceMapOptions::default());
  assert_eq!(sm.to_json().mappings, "AAAA,MAAS");
}

#[test]
fn hires_boundary_maps_word_at_start_of_next_line() {
  // The first line ends with a word char and the second starts with one.
  let s = MagicString::new("const a = 1\nconst b = 2");
  let sm = s.source_map(SourceMapOptions { hires: Hires::Boundary, ..Default::default() });
  assert!(
    sm.get_tokens().any(|t| t.get_dst_line() == 1
      && t.get_dst_col() == 0
      && t.get_src_line() == 1
      && t.get_src_col() == 0),
    "the word starting the line after a word-ending line lost its source mapping"
  );
}
