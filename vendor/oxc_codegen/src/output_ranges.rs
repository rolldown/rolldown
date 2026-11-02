//! Optional output-fragment accounting for Rolldown's retained complete programs.
//! Owners describe emitted fragments, not sourcemap/source contributions.

use std::ops::Range;

use oxc_span::Span;

use crate::Codegen;

/// A source-space interval identifying one donor. Intervals must be sorted and disjoint.
#[derive(Debug, Clone)]
pub struct OutputRangeOwner {
    pub source: Range<u32>,
    pub owner: u32,
}

/// An actual byte range in the final generated code. Ranges never overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRange {
    pub owner: u32,
    pub output: Range<usize>,
}

pub(crate) struct OutputRanges {
    owners: Vec<OutputRangeOwner>,
    ranges: Vec<OutputRange>,
    active: Option<u32>,
    pending_semicolon: Option<u32>,
    cursor: usize,
}

impl OutputRanges {
    pub fn new(owners: Vec<OutputRangeOwner>) -> Self {
        debug_assert!(owners.windows(2).all(|pair| pair[0].source.end <= pair[1].source.start));
        Self { owners, ranges: Vec::new(), active: None, pending_semicolon: None, cursor: 0 }
    }

    fn flush(&mut self, end: usize) {
        if end > self.cursor
            && let Some(owner) = self.active
        {
            if let Some(last) = self.ranges.last_mut()
                && last.owner == owner
                && last.output.end == self.cursor
            {
                last.output.end = end;
            } else {
                self.ranges.push(OutputRange { owner, output: self.cursor..end });
            }
        }
        self.cursor = end;
    }

    fn owner(&self, span: Span) -> Option<u32> {
        // Minifier-created SPAN has no producer. A real donor's rebased empty span
        // can otherwise name its interval, including an empty input module.
        if span == oxc_span::SPAN {
            return None;
        }
        let index =
            self.owners.partition_point(|item| item.source.start <= span.start).checked_sub(1)?;
        let item = &self.owners[index];
        (span.start < item.source.end
            || (item.source.is_empty() && span.start == item.source.start))
            .then_some(item.owner)
    }
}

impl Codegen<'_> {
    /// Record donor fragments without affecting printing, source maps or syntax decisions.
    #[must_use]
    pub fn with_output_range_owners(mut self, owners: Vec<OutputRangeOwner>) -> Self {
        self.output_ranges = Some(OutputRanges::new(owners));
        self
    }

    pub(crate) fn begin_output_fragment(&mut self, span: Span) -> bool {
        let Some(ranges) = &mut self.output_ranges else {
            return false;
        };
        if ranges.active.is_some() {
            return false;
        }
        let Some(owner) = ranges.owner(span) else {
            return false;
        };
        ranges.flush(self.code.len());
        ranges.active = Some(owner);
        true
    }

    pub(crate) fn end_output_fragment(&mut self, claimed: bool) {
        if !claimed {
            return;
        }
        let ranges = self.output_ranges.as_mut().unwrap();
        ranges.flush(self.code.len());
        ranges.active = None;
    }

    pub(crate) fn defer_output_semicolon(&mut self) {
        if let Some(ranges) = &mut self.output_ranges {
            ranges.pending_semicolon = ranges.active;
        }
    }

    pub(crate) fn begin_output_semicolon(&mut self) -> Option<u32> {
        let Some(ranges) = &mut self.output_ranges else {
            return None;
        };
        ranges.flush(self.code.len());
        std::mem::replace(&mut ranges.active, ranges.pending_semicolon.take())
    }

    pub(crate) fn end_output_semicolon(&mut self, previous: Option<u32>) {
        if let Some(ranges) = &mut self.output_ranges {
            ranges.flush(self.code.len());
            ranges.active = previous;
        }
    }

    pub(crate) fn finish_output_ranges(&mut self) -> Vec<OutputRange> {
        self.output_ranges.take().map_or_else(Vec::new, |mut ranges| {
            ranges.flush(self.code.len());
            ranges.ranges
        })
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Expression, Statement};
    use oxc_parser::Parser;
    use oxc_span::{SPAN, SourceType, Span};

    use crate::{Codegen, CodegenOptions, OutputRangeOwner};

    fn fragments(code: &str, ranges: &[crate::OutputRange], owner: u32) -> String {
        ranges
            .iter()
            .filter(|range| range.owner == owner)
            .map(|range| &code[range.output.clone()])
            .collect()
    }

    #[test]
    fn nested_frontiers_preserve_code_maps_and_pending_semicolons() {
        let source = "function generated(){first('😀');second()}after();";
        let first = source.find("first").unwrap();
        let second = source.find("second").unwrap();
        let end = source.find('}').unwrap();
        for minify in [false, true] {
            for maps in [false, true] {
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
                let options = CodegenOptions {
                    minify,
                    source_map_path: maps.then(|| "test.js".into()),
                    ..Default::default()
                };
                let plain = Codegen::new().with_options(options.clone()).build(&parsed.program);
                let traced = Codegen::new()
                    .with_options(options)
                    .with_output_range_owners(vec![
                        OutputRangeOwner { source: first as u32..second as u32, owner: 0 },
                        OutputRangeOwner { source: second as u32..end as u32, owner: 1 },
                    ])
                    .build(&parsed.program);
                assert_eq!(plain.code, traced.code);
                assert_eq!(
                    plain.map.as_ref().map(|map| map.to_json_string()),
                    traced.map.as_ref().map(|map| map.to_json_string())
                );
                assert!(
                    traced
                        .output_ranges
                        .windows(2)
                        .all(|pair| pair[0].output.end <= pair[1].output.start)
                );
                let first = fragments(&traced.code, &traced.output_ranges, 0);
                let second = fragments(&traced.code, &traced.output_ranges, 1);
                assert!(first.contains("first("));
                assert!(first.contains('😀'));
                assert!(first.ends_with(if minify { ";" } else { ";\n" }), "{first:?}");
                assert!(second.contains("second()"));
                assert!(!(first.clone() + &second).contains("generated"));
                assert!(!(first + &second).contains("after"));
            }
        }
    }

    #[test]
    fn merged_consumer_frontier_keeps_moved_child() {
        let source = "consumer('producer');";
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        let traced = Codegen::new()
            .with_output_range_owners(vec![
                OutputRangeOwner { source: 0..9, owner: 0 },
                OutputRangeOwner { source: 9..source.len() as u32, owner: 1 },
            ])
            .build(&parsed.program);
        assert!(traced.output_ranges.iter().all(|range| range.owner == 0));
        assert_eq!(traced.output_ranges[0].output, 0..traced.code.len());
    }

    #[test]
    fn iterative_binary_frontiers_find_donor_operands() {
        let source = "a + b + c;";
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        let Statement::ExpressionStatement(statement) = &mut parsed.program.body[0] else {
            unreachable!();
        };
        statement.span = SPAN;
        let Expression::BinaryExpression(root) = &mut statement.expression else {
            unreachable!();
        };
        root.span = SPAN;
        let Expression::BinaryExpression(left) = &mut root.left else { unreachable!() };
        left.span = SPAN;
        let traced = Codegen::new()
            .with_options(CodegenOptions { minify: true, ..Default::default() })
            .with_output_range_owners(vec![
                OutputRangeOwner { source: 0..1, owner: 0 },
                OutputRangeOwner { source: 4..5, owner: 1 },
                OutputRangeOwner { source: 8..9, owner: 2 },
            ])
            .build(&parsed.program);
        for (owner, expected) in ["a", "b", "c"].into_iter().enumerate() {
            assert_eq!(fragments(&traced.code, &traced.output_ranges, owner as u32), expected);
        }
    }

    #[test]
    fn direct_cjs_literal_printers_still_claim_donors() {
        for (source, literal) in [
            ("key === 'default';", "'default'"),
            ("require('donor');", "'donor'"),
            ("Object.defineProperty(exports, 'name', value);", "'name'"),
            ("exports['name'] = value;", "'name'"),
        ] {
            let start = source.find(literal).unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
            let traced = Codegen::new()
                .with_options(CodegenOptions { minify: true, ..Default::default() })
                .with_output_range_owners(vec![OutputRangeOwner {
                    source: start as u32..(start + literal.len()) as u32,
                    owner: 0,
                }])
                .build(&parsed.program);
            let expected = format!("\"{}\"", &literal[1..literal.len() - 1]);
            assert_eq!(fragments(&traced.code, &traced.output_ranges, 0), expected, "{source}");
        }
    }

    #[test]
    fn orphan_comments_include_their_layout() {
        let source = "/*first*/\nfunction generated() {\n/*second*/\n}\nafter();";
        let first = source.find("/*first*/").unwrap();
        let second = source.find("/*second*/").unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
        let traced = Codegen::new()
            .with_output_range_owners(vec![
                OutputRangeOwner { source: first as u32..(first + 9) as u32, owner: 0 },
                OutputRangeOwner { source: second as u32..(second + 10) as u32, owner: 1 },
            ])
            .build(&parsed.program);
        for owner in 0..2 {
            let fragment = fragments(&traced.code, &traced.output_ranges, owner);
            assert!(fragment.contains(if owner == 0 { "/*first*/" } else { "/*second*/" }));
            assert!(fragment.ends_with('\n'), "{fragment:?}");
        }
    }

    #[test]
    fn zero_span_never_implies_the_first_donor() {
        let ranges = super::OutputRanges::new(vec![OutputRangeOwner { source: 1..2, owner: 0 }]);
        assert_eq!(ranges.owner(Span::new(0, 0)), None);
        assert_eq!(ranges.owner(Span::new(1, 1)), Some(0));
    }
}
