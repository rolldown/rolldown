//! Optional source provenance for literal inlining. Kept out of dense SymbolValue storage.

use oxc_ast::ast::{Expression, Program};
use oxc_ast_visit::{Visit, walk};
use oxc_index::IndexVec;
use oxc_span::{GetSpan, SPAN, Span};
use oxc_syntax::symbol::SymbolId;

pub struct LiteralSpans {
    original: Vec<Span>,
    comment_anchors: Vec<u32>,
    // Allocated on the first actual donor; retained and cleared between passes.
    symbols: Option<IndexVec<SymbolId, Span>>,
    symbols_len: usize,
}

impl LiteralSpans {
    pub fn new(program: &Program<'_>, symbols_len: usize) -> Self {
        struct Collector(Vec<Span>);
        impl<'a> Visit<'a> for Collector {
            fn visit_expression(&mut self, expr: &Expression<'a>) {
                if is_literal(expr) && !expr.span().is_empty() {
                    self.0.push(expr.span());
                }
                walk::walk_expression(self, expr);
            }
        }
        let mut collector = Collector(Vec::new());
        collector.visit_program(program);
        collector.0.sort_unstable();
        collector.0.dedup();
        let mut comment_anchors: Vec<_> = program.comments.iter().map(|c| c.attached_to).collect();
        comment_anchors.sort_unstable();
        comment_anchors.dedup();
        Self { original: collector.0, comment_anchors, symbols: None, symbols_len }
    }

    pub fn reset(&mut self) {
        if let Some(symbols) = &mut self.symbols {
            symbols.raw.fill(SPAN);
        }
    }

    pub fn original_span(&self, expr: &Expression<'_>) -> Option<Span> {
        let span = expr.span();
        (is_literal(expr) && self.original.binary_search(&span).is_ok()).then_some(span)
    }

    pub fn symbol_span(&self, symbol: SymbolId) -> Option<Span> {
        let span = *self.symbols.as_ref()?.get(symbol)?;
        (!span.is_empty()).then_some(span)
    }

    pub fn set_symbol_span(&mut self, symbol: SymbolId, span: Option<Span>) {
        if let Some(span) = span {
            let symbols = self
                .symbols
                .get_or_insert_with(|| IndexVec::from_vec(vec![SPAN; self.symbols_len]));
            symbols[symbol] = span;
        } else if let Some(symbols) = &mut self.symbols {
            symbols[symbol] = SPAN;
        }
    }

    pub fn can_replace_span(&self, consumer: Span, donor: Span) -> bool {
        // All comment kinds are guarded, including trailing anchors. Keeping the
        // old consumer span preserves #8248 without mutating shared comment data.
        // A donor anchor could duplicate a comment at multiple constant reads.
        [consumer.start, consumer.end, donor.start, donor.end]
            .iter()
            .all(|anchor| self.comment_anchors.binary_search(anchor).is_err())
    }
}

fn is_literal(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
    ) || matches!(expr, Expression::TemplateLiteral(t) if t.expressions.is_empty())
}
