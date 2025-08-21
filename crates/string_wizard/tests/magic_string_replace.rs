use oxc_sourcemap::SourcemapVisualizer;
use string_wizard::{MagicString, ReplaceOptions, SourceMapOptions};

#[test]
fn works_with_string_replace() {
  let code = "1 2 1 2";
  let mut s = MagicString::new(code);

  s.replace("2", "3");

  assert_eq!(s.to_string(), "1 3 1 2");
}

#[test]
fn should_not_search_back() {
  let code = "122121";
  let mut s = MagicString::new(code);

  s.replace("12", "21");

  assert_eq!(s.to_string(), "212121");
}

mod replace_all {
  use super::*;

  #[test]
  fn works_with_string_replace() {
    let code = "1212";
    let mut s = MagicString::new(code);

    s.replace_all("2", "3");

    assert_eq!(s.to_string(), "1313");
  }

  #[test]
  fn should_not_search_back() {
    let code = "121212";
    let mut s = MagicString::new(code);

    s.replace_all("12", "21");

    assert_eq!(s.to_string(), "212121");
  }
}

#[test]
fn replace_sourcemap_with_chinese() {
  let code = "测 2 测 4";
  let mut s = MagicString::new(code);

  s.replace_with(
    "测",
    "试",
    ReplaceOptions { count: usize::MAX, store_original_in_sourcemap: true },
  );

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

  s.replace_with("*", "#", ReplaceOptions { count: usize::MAX, store_original_in_sourcemap: true });

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
