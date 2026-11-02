use std::sync::Arc;

use oxc::codegen::LineOffsetTables;
use oxc_sourcemap::{SourceMap, SourceMapParts, Token};
use rolldown_common::{ModuleIdx, ModuleTable};

use super::{ChunkSourceSpace, SpaceModule, is_cjs_runtime_statement, remap};

#[test]
fn cjs_runtime_ownership_excludes_synthesized_span_statements() {
  assert!(is_cjs_runtime_statement(true, 0, oxc::span::Span::new(10, 20)));
  assert!(!is_cjs_runtime_statement(true, 0, oxc::span::SPAN));
  assert!(!is_cjs_runtime_statement(false, 0, oxc::span::SPAN));
}

#[test]
fn whole_program_remap_preserves_source_order_and_reuses_ids() {
  let space = ChunkSourceSpace {
    text: Arc::from("a\nv\nb\nc\n"),
    tables: Arc::new(LineOffsetTables::default()),
    modules: (0_u32..4)
      .map(|index| SpaceModule {
        base: index * 2,
        len: 1,
        line: index,
        end_line: index,
        source_name: match index {
          0 => Some("a.js".into()),
          2 => Some("b.js".into()),
          3 => Some("c.js".into()),
          _ => None,
        },
        idx: ModuleIdx::from_raw(index),
        has_chain: false,
      })
      .collect(),
    scoping: None,
    private_member_mappings: None,
    class_offsets: vec![0; 4],
  };
  let map = SourceMap::from(SourceMapParts {
    sources: vec!["V".into()],
    tokens: [3, 1, 0, 2, 3, 1, 2, 0]
      .into_iter()
      .zip(0_u32..)
      .map(|(line, column)| Token::new(0, column, line, 0, Some(0), None))
      .collect(),
    ..Default::default()
  });
  let (map, is_final) = remap(&map, &space, None, &ModuleTable::default());
  assert!(is_final);
  // Reparse the serialized map to check source IDs and explicit unmapped segments together.
  let serialized = map.to_json_string();
  let map = SourceMap::from_json_string(&serialized).unwrap();
  assert_eq!(map.get_sources().collect::<Vec<_>>(), ["c.js", "a.js", "b.js"]);
  assert_eq!(
    map.get_tokens().map(|token| token.get_source_id()).collect::<Vec<_>>(),
    [Some(0), None, Some(1), Some(2), Some(0), None, Some(2), Some(1)]
  );
}

#[test]
fn remap_preserves_unmapped_boundaries() {
  let space = ChunkSourceSpace {
    text: Arc::from("real\nvirtual\ngenerated\n"),
    tables: Arc::new(LineOffsetTables::default()),
    modules: vec![
      SpaceModule {
        base: 0,
        len: 4,
        line: 0,
        end_line: 0,
        source_name: Some("real.js".into()),
        idx: ModuleIdx::from_raw(0),
        has_chain: false,
      },
      SpaceModule {
        base: 5,
        len: 7,
        line: 1,
        end_line: 1,
        source_name: None,
        idx: ModuleIdx::from_raw(1),
        has_chain: false,
      },
    ],
    scoping: None,
    private_member_mappings: None,
    class_offsets: vec![0, 0],
  };
  let map = SourceMap::from(SourceMapParts {
    sources: vec!["V".into()],
    tokens: vec![
      Token::new(0, 0, 0, 0, Some(0), None),
      Token::new(0, 4, 1, 0, Some(0), None),
      Token::new(0, 8, 0, 1, Some(0), None),
      Token::new(0, 12, 2, 0, Some(0), None),
      Token::new(0, 16, 0, 2, Some(0), None),
      Token::new(0, 20, 0, 0, None, None),
    ]
    .into_boxed_slice(),
    ..Default::default()
  });
  for owner in [None, Some(0), Some(1)] {
    let (map, _) = remap(&map, &space, owner, &ModuleTable::default());
    let lookup = map.generate_lookup_table();
    for column in [4, 12, 20] {
      let token = map.lookup_source_view_token(&lookup, 0, column + 1).unwrap();
      assert_eq!(token.get_source_id(), None, "owner={owner:?}, column={column}");
    }
    for (column, source_column) in [(0, 0), (8, 1), (16, 2)] {
      let token = map.lookup_source_view_token(&lookup, 0, column + 1).unwrap();
      assert_eq!(token.get_source_id(), Some(0));
      assert_eq!(token.get_src_col(), source_column);
    }
  }
}
