use crate::SourceMap;

/// Sentinel value representing an invalid/missing ID for source or name.
/// Used when a token doesn't have an associated source file or name.
pub(crate) const INVALID_ID: u32 = u32::MAX;

/// The `Token` is used to generate vlq `mappings`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Token {
    pub(crate) dst_line: u32,
    pub(crate) dst_col: u32,
    pub(crate) src_line: u32,
    pub(crate) src_col: u32,
    source_id: u32,
    name_id: u32,
}

impl Token {
    pub fn new(
        dst_line: u32,
        dst_col: u32,
        src_line: u32,
        src_col: u32,
        source_id: Option<u32>,
        name_id: Option<u32>,
    ) -> Self {
        Self {
            dst_line,
            dst_col,
            src_line,
            src_col,
            source_id: source_id.unwrap_or(INVALID_ID),
            name_id: name_id.unwrap_or(INVALID_ID),
        }
    }

    /// Translate this token for a concatenated source map: shift the generated line by
    /// `line_offset` and renumber the source/name ids by `source_offset` / `name_offset`,
    /// preserving the missing-id sentinel. Operates on the raw ids so there is no `Option`
    /// round-trip in the concat hot loop.
    #[inline]
    pub(crate) fn translated(self, line_offset: u32, source_offset: u32, name_offset: u32) -> Self {
        let shift = |id: u32, offset: u32| if id == INVALID_ID { INVALID_ID } else { id + offset };
        Self {
            dst_line: self.dst_line + line_offset,
            source_id: shift(self.source_id, source_offset),
            name_id: shift(self.name_id, name_offset),
            ..self
        }
    }

    #[inline]
    pub fn get_dst_line(&self) -> u32 {
        self.dst_line
    }

    #[inline]
    pub fn get_dst_col(&self) -> u32 {
        self.dst_col
    }

    #[inline]
    pub fn get_src_line(&self) -> u32 {
        self.src_line
    }

    #[inline]
    pub fn get_src_col(&self) -> u32 {
        self.src_col
    }

    #[inline]
    pub fn get_name_id(&self) -> Option<u32> {
        if self.name_id == INVALID_ID { None } else { Some(self.name_id) }
    }

    #[inline]
    pub fn get_source_id(&self) -> Option<u32> {
        if self.source_id == INVALID_ID { None } else { Some(self.source_id) }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenChunk {
    pub start: u32,
    pub end: u32,
    pub prev_dst_line: u32,
    pub prev_dst_col: u32,
    pub prev_src_line: u32,
    pub prev_src_col: u32,
    pub prev_name_id: u32,
    pub prev_source_id: u32,
}

impl TokenChunk {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        start: u32,
        end: u32,
        prev_dst_line: u32,
        prev_dst_col: u32,
        prev_src_line: u32,
        prev_src_col: u32,
        prev_name_id: u32,
        prev_source_id: u32,
    ) -> Self {
        Self {
            start,
            end,
            prev_dst_line,
            prev_dst_col,
            prev_src_line,
            prev_src_col,
            prev_name_id,
            prev_source_id,
        }
    }
}

/// The `SourceViewToken` provider extra `source` and `source_content` value.
///
/// Two lifetimes:
/// * `'sm` — the borrow of the [`SourceMap`] reference itself.
/// * `'data` — the underlying string data inside the [`SourceMap`] (the input
///   JSON buffer, for maps parsed via [`SourceMap::from_json_string`]).
#[derive(Debug, Clone, Copy)]
pub struct SourceViewToken<'sm, 'data> {
    pub(crate) token: Token,
    pub(crate) sourcemap: &'sm SourceMap<'data>,
}

impl<'sm, 'data> SourceViewToken<'sm, 'data> {
    pub fn new(token: Token, sourcemap: &'sm SourceMap<'data>) -> Self {
        Self { token, sourcemap }
    }

    pub fn get_dst_line(&self) -> u32 {
        self.token.dst_line
    }

    pub fn get_dst_col(&self) -> u32 {
        self.token.dst_col
    }

    pub fn get_src_line(&self) -> u32 {
        self.token.src_line
    }

    pub fn get_src_col(&self) -> u32 {
        self.token.src_col
    }

    pub fn get_name_id(&self) -> Option<u32> {
        self.token.get_name_id()
    }

    pub fn get_source_id(&self) -> Option<u32> {
        self.token.get_source_id()
    }

    pub fn get_name(&self) -> Option<&'sm str> {
        self.get_name_id().and_then(|id| self.sourcemap.get_name(id))
    }

    pub fn get_source(&self) -> Option<&'sm str> {
        self.get_source_id().and_then(|id| self.sourcemap.get_source(id))
    }

    pub fn get_source_content(&self) -> Option<&'sm str> {
        self.get_source_id().and_then(|id| self.sourcemap.get_source_content(id))
    }

    pub fn get_source_and_content(&self) -> Option<(&'sm str, &'sm str)> {
        self.get_source_id().and_then(|id| self.sourcemap.get_source_and_content(id))
    }

    #[expect(clippy::wrong_self_convention)]
    pub fn to_tuple(&self) -> (Option<&'sm str>, u32, u32, Option<&'sm str>) {
        (self.get_source(), self.get_src_line(), self.get_src_col(), self.get_name())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    fn sample_map() -> SourceMap<'static> {
        SourceMap::new(
            None,
            vec![Cow::Borrowed("name0")],
            None,
            vec![Cow::Borrowed("src0.js")],
            vec![Some(Cow::Borrowed("the source content"))],
            vec![Token::new(2, 3, 4, 5, Some(0), Some(0))].into_boxed_slice(),
            None,
        )
    }

    #[test]
    fn token_getters() {
        let token = Token::new(1, 2, 3, 4, Some(5), Some(6));
        assert_eq!(token.get_dst_line(), 1);
        assert_eq!(token.get_dst_col(), 2);
        assert_eq!(token.get_src_line(), 3);
        assert_eq!(token.get_src_col(), 4);
        assert_eq!(token.get_source_id(), Some(5));
        assert_eq!(token.get_name_id(), Some(6));

        let missing = Token::new(0, 0, 0, 0, None, None);
        assert_eq!(missing.get_source_id(), None);
        assert_eq!(missing.get_name_id(), None);
    }

    #[test]
    fn token_translated() {
        let token = Token::new(1, 2, 3, 4, Some(5), Some(6));
        assert_eq!(token.translated(10, 100, 1000), Token::new(11, 2, 3, 4, Some(105), Some(1006)));

        // The missing-id sentinel survives translation rather than wrapping.
        let missing = Token::new(0, 0, 0, 0, None, None);
        let shifted = missing.translated(5, 5, 5);
        assert_eq!(shifted.get_dst_line(), 5);
        assert_eq!(shifted.get_source_id(), None);
        assert_eq!(shifted.get_name_id(), None);
    }

    #[test]
    fn source_view_token_accessors() {
        let sm = sample_map();
        let token = sm.get_source_view_token(0).unwrap();
        assert_eq!(token.get_dst_line(), 2);
        assert_eq!(token.get_dst_col(), 3);
        assert_eq!(token.get_src_line(), 4);
        assert_eq!(token.get_src_col(), 5);
        assert_eq!(token.get_source_id(), Some(0));
        assert_eq!(token.get_name_id(), Some(0));
        assert_eq!(token.get_name(), Some("name0"));
        assert_eq!(token.get_source(), Some("src0.js"));
        assert_eq!(token.get_source_content(), Some("the source content"));
        assert_eq!(token.get_source_and_content(), Some(("src0.js", "the source content")));
        assert_eq!(token.to_tuple(), (Some("src0.js"), 4, 5, Some("name0")));
    }

    #[test]
    fn source_view_token_without_ids() {
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![Token::new(0, 0, 0, 0, None, None)].into_boxed_slice(),
            None,
        );
        let token = sm.get_source_view_token(0).unwrap();
        assert_eq!(token.get_name(), None);
        assert_eq!(token.get_source(), None);
        assert_eq!(token.get_source_content(), None);
        assert_eq!(token.get_source_and_content(), None);
    }
}
