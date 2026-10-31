use oxc_sourcemap::SourceMap;

pub(super) fn position(text: &str, byte: usize) -> (u32, u32) {
  let prefix = &text[..byte];
  (
    u32::try_from(prefix.bytes().filter(|&byte| byte == b'\n').count()).unwrap(),
    u32::try_from(prefix.rsplit('\n').next().unwrap().encode_utf16().count()).unwrap(),
  )
}

pub(super) fn assert_mapping(
  code: &str,
  map: &SourceMap<'_>,
  generated: &str,
  source: &str,
  content: &str,
  original: &str,
) {
  let byte = code
    .find(generated)
    .unwrap_or_else(|| panic!("missing generated anchor {generated:?} in {code}"));
  assert_mapping_at(code, map, byte, source, content, content.find(original).unwrap());
}

pub(super) fn assert_mapping_at(
  code: &str,
  map: &SourceMap<'_>,
  generated: usize,
  source: &str,
  content: &str,
  original: usize,
) {
  let (line, column) = position(code, generated);
  let lookup = map.generate_lookup_table();
  let token = map
    .lookup_source_view_token(&lookup, line, column)
    .unwrap_or_else(|| panic!("missing mapping at {line}:{column}"));
  let source_id =
    token.get_source_id().unwrap_or_else(|| panic!("unmapped {line}:{column}, expected {source}"));
  assert!(
    map.get_source(source_id).unwrap().ends_with(source),
    "wrong producer at {line}:{column}: {token:?}"
  );
  assert_eq!(
    map.get_source_content(source_id),
    Some(content),
    "must retain original source content"
  );
  assert_eq!(
    (token.get_src_line(), token.get_src_col()),
    position(content, original),
    "wrong original position at {line}:{column}: {token:?}"
  );
}

pub(super) fn assert_valid_map(code: &str, map: &SourceMap<'_>) {
  let generated_lines: Vec<_> = code.split('\n').collect();
  let mut previous = None;
  for token in map.get_source_view_tokens() {
    let generated = (token.get_dst_line(), token.get_dst_col());
    assert!(previous.is_none_or(|previous| previous <= generated), "mapping order: {token:?}");
    previous = Some(generated);
    let line = generated_lines[generated.0 as usize];
    assert!(
      generated.1 as usize <= line.encode_utf16().count(),
      "generated column out of range: {token:?}"
    );
    if let Some(source_id) = token.get_source_id() {
      let content =
        map.get_source_content(source_id).expect("mapped producer needs original content");
      let line =
        content.split('\n').nth(token.get_src_line() as usize).expect("original line out of range");
      assert!(
        token.get_src_col() as usize <= line.encode_utf16().count(),
        "original column out of range: {token:?}"
      );
      if let Some(name) = token.get_name() {
        let original = String::from_utf16(
          &line.encode_utf16().skip(token.get_src_col() as usize).collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(original.starts_with(name), "name must match original spelling: {token:?}");
      }
    }
  }
}

pub(super) fn assert_unmapped(code: &str, map: &SourceMap<'_>, generated: &str) {
  let (line, column) = position(code, code.find(generated).unwrap());
  let lookup = map.generate_lookup_table();
  let token = map.lookup_source_view_token(&lookup, line, column);
  assert!(
    token.is_none_or(|token| token.get_source_id().is_none()),
    "generated text must not inherit a source mapping: {token:?}"
  );
}
