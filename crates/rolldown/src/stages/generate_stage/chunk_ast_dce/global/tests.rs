use std::sync::Arc;

use oxc::codegen::LineOffsetTables;
use oxc_sourcemap::{SourceMap, SourceMapParts, Token};
use rolldown_common::{ModuleIdx, ModuleTable};

use super::{ChunkSourceSpace, SpaceModule, remap};

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
