use std::path::PathBuf;

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::{SPAN, SourceType};

use super::{Codegen, CodegenOptions};

#[test]
fn synthesized_block_closing_ends_child_mapping() {
    let allocator = Allocator::default();
    let source = "function wrapper(){real()}after();";
    let mut parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    assert!(parsed.diagnostics.is_empty());
    let Statement::FunctionDeclaration(function) = &mut parsed.program.body[0] else {
        panic!("expected function");
    };
    // Models the generated CommonJS wrapper around statements retaining real spans.
    function.body.as_mut().unwrap().span = SPAN;
    let printed = Codegen::new()
        .with_options(CodegenOptions {
            minify: true,
            source_map_path: Some(PathBuf::from("input.js")),
            ..CodegenOptions::default()
        })
        .build(&parsed.program);
    let map = printed.map.unwrap();
    let lookup = map.generate_lookup_table();
    let real = printed.code.find("real").unwrap() as u32;
    let closing = printed.code.find('}').unwrap() as u32;
    let after = printed.code.find("after").unwrap() as u32;
    let real_token = map.lookup_source_view_token(&lookup, 0, real).unwrap();
    assert_eq!(real_token.get_src_col(), source.find("real").unwrap() as u32);
    assert!(real_token.get_source_id().is_some());
    let closing_token = map.lookup_source_view_token(&lookup, 0, closing).unwrap();
    assert_eq!(closing_token.get_source_id(), None);
    let after_token = map.lookup_source_view_token(&lookup, 0, after).unwrap();
    assert_eq!(after_token.get_src_col(), source.find("after").unwrap() as u32);
    assert!(after_token.get_source_id().is_some());
}
