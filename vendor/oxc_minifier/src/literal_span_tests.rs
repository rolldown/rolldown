use std::path::PathBuf;

use oxc_allocator::Allocator;
use oxc_ast::ast::StringLiteral;
use oxc_ast_visit::Visit;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

use crate::{CompressOptions, Minifier, MinifierOptions};

#[derive(Default)]
struct Strings(Vec<Span>);

impl<'a> Visit<'a> for Strings {
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.0.push(literal.span);
    }
}

fn run(
    source: &str,
    preserve: bool,
    dce: bool,
    sourcemap: bool,
) -> (Vec<Span>, String, Option<u32>) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let mut program = parsed.program;
    let minifier = Minifier::new(MinifierOptions {
        compress: Some(CompressOptions::default()),
        mangle: None,
        mangle_properties: None,
    })
    .with_literal_spans(preserve);
    if dce {
        minifier.dce(&allocator, &mut program);
    } else {
        minifier.minify(&allocator, &mut program);
    }
    let mut strings = Strings::default();
    strings.visit_program(&program);
    let printed = Codegen::new()
        .with_options(CodegenOptions {
            minify: true,
            source_map_path: sourcemap.then(|| PathBuf::from("input.js")),
            ..CodegenOptions::default()
        })
        .build(&program);
    let mapped_col = printed.map.as_ref().map(|map| {
        let col = u32::try_from(
            printed
                .code
                .find(['\'', '"', '`'])
                .unwrap_or_else(|| panic!("no string in {:?}", printed.code)),
        )
        .unwrap();
        let lookup = map.generate_lookup_table();
        map.lookup_source_view_token(&lookup, 0, col)
            .unwrap_or_else(|| {
                panic!(
                    "no token at {col}: {:?} / {:?}",
                    printed.code,
                    map.get_tokens().collect::<Vec<_>>()
                )
            })
            .get_src_col()
    });
    (strings.0, printed.code, mapped_col)
}

#[test]
fn literal_span_donor_and_aliases() {
    for source in [
        "const shader='unique shader source'; export function get(){return shader}",
        "const shader='unique shader source'; const alias=shader; const other=alias; export function get(){return other}",
        "const shader=`unique shader source`; export function get(){return shader}",
    ] {
        let donor = u32::try_from(source.find(['\'', '`']).unwrap()).unwrap();
        let (spans, _, mapped) = run(source, true, false, true);
        assert_eq!(spans.len(), 1, "{source}");
        assert_eq!(spans[0].start, donor, "{source}");
        assert_eq!(mapped, Some(donor), "{source}");
    }
}

#[test]
fn literal_span_single_use_move() {
    let source = "const shader='unique shader source'; consume(shader);";
    for dce in [false, true] {
        let (spans, _, mapped) = run(source, true, dce, true);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, u32::try_from(source.find('\'').unwrap()).unwrap());
        assert_eq!(mapped, Some(spans[0].start));
    }
}

#[test]
fn literal_span_synthesized_fold_uses_consumer() {
    for source in [
        "const shader='unique '+'shader source'; export function get(){return shader}",
        "const shader=`unique ${'shader'} source`; export function get(){return shader}",
        "const shader='unique '+'shader source'; consume(shader);",
    ] {
        let (spans, _, mapped) = run(source, true, false, true);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            spans[0].start,
            u32::try_from(source.rfind("shader").unwrap()).unwrap(),
            "{source}"
        );
        assert_eq!(mapped, Some(spans[0].start));
    }
}

#[test]
fn literal_span_consumer_comments_take_precedence() {
    for annotation in [
        "/* @vite-ignore */",
        "/* webpackIgnore: true */",
        "/* @vite-ignore */ /* webpackIgnore: true */",
    ] {
        let source = format!("const shader='some-url'; import({annotation} shader);");
        for dce in [false, true] {
            let (spans, code, _) = run(&source, true, dce, true);
            for comment in annotation.split(" */ ") {
                assert!(code.contains(comment), "{code}");
            }
            assert_eq!(spans[0].start, u32::try_from(source.rfind("shader").unwrap()).unwrap());
        }
    }
}

#[test]
fn literal_span_default_off_and_no_sourcemap() {
    assert!(!Minifier::new(MinifierOptions::default()).preserve_literal_spans);
    let source = "const shader='unique shader source'; consume(shader);";
    for sourcemap in [false, true] {
        let (spans, default_code, _) = run(source, false, false, sourcemap);
        assert_eq!(spans[0].start, u32::try_from(source.rfind("shader").unwrap()).unwrap());
        let (_, preserved_code, _) = run(source, true, false, sourcemap);
        assert_eq!(default_code, preserved_code);
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn literal_span_storage_sizes() {
    use crate::{symbol_state::SymbolState, symbol_value::SymbolValue};
    use std::mem::size_of;
    // Keep dense symbol storage at its measured pre-patch sizes.
    assert_eq!(size_of::<SymbolValue>(), 48);
    assert_eq!(size_of::<Option<SymbolValue>>(), 48);
    assert_eq!(size_of::<SymbolState>(), 248);
    assert_eq!(size_of::<Span>(), 8);
}
