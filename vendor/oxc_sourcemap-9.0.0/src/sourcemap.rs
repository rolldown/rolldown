use std::borrow::Cow;

use crate::{
    SourceViewToken,
    decode::{JSONSourceMap, decode, decode_from_string},
    encode::{encode, encode_to_string},
    error::Result,
    token::{Token, TokenChunk},
};

/// A parsed source map.
///
/// `SourceMap` is parameterized by a lifetime `'a` because its string fields
/// borrow directly from the source they were built from when possible. For a
/// map parsed via [`SourceMap::from_json_string`], `'a` is the lifetime of the
/// input JSON buffer — most strings (those without JSON escapes) are zero-copy
/// views into that buffer; only escaped strings allocate. For maps built
/// programmatically via the builder, the lifetime is `'static`.
#[derive(Debug, Clone, Default)]
pub struct SourceMap<'a> {
    pub(crate) file: Option<Cow<'a, str>>,
    pub(crate) names: Vec<Cow<'a, str>>,
    pub(crate) source_root: Option<Cow<'a, str>>,
    pub(crate) sources: Vec<Cow<'a, str>>,
    pub(crate) source_contents: Vec<Option<Cow<'a, str>>>,
    pub(crate) tokens: Box<[Token]>,
    pub(crate) token_chunks: Option<Vec<TokenChunk>>,
    /// Identifies third-party sources (such as framework code or bundler-generated code), allowing developers to avoid code that they don't want to see or step through, without having to configure this beforehand.
    /// The `ignoreList` field refers to the `sources` array, and lists the indices of all the known third-party sources in that source map.
    /// When parsing the source map, developer tools can use this to determine sections of the code that the browser loads and runs that could be automatically ignore-listed.
    pub(crate) ignore_list: Option<Vec<u32>>,
    pub(crate) debug_id: Option<Cow<'a, str>>,
}

impl<'a> SourceMap<'a> {
    pub fn new(
        file: Option<Cow<'a, str>>,
        names: Vec<Cow<'a, str>>,
        source_root: Option<Cow<'a, str>>,
        sources: Vec<Cow<'a, str>>,
        source_contents: Vec<Option<Cow<'a, str>>>,
        tokens: Box<[Token]>,
        token_chunks: Option<Vec<TokenChunk>>,
    ) -> Self {
        Self {
            file,
            names,
            source_root,
            sources,
            source_contents,
            tokens,
            token_chunks,
            ignore_list: None,
            debug_id: None,
        }
    }

    /// Convert the vlq sourcemap to `SourceMap`.
    /// # Errors
    ///
    /// The `serde_json` deserialize Error.
    pub fn from_json(value: JSONSourceMap) -> Result<SourceMap<'static>> {
        decode(value)
    }

    /// Convert the vlq sourcemap string to `SourceMap`.
    ///
    /// The returned `SourceMap` borrows string data from `value` for any
    /// fields that have no JSON escape sequences; everything else is owned.
    ///
    /// # Errors
    ///
    /// The `serde_json` deserialize Error.
    pub fn from_json_string(value: &'a str) -> Result<SourceMap<'a>> {
        decode_from_string(value)
    }

    /// Convert `SourceMap` to vlq sourcemap.
    pub fn to_json(&self) -> JSONSourceMap {
        encode(self)
    }

    /// Convert `SourceMap` to vlq sourcemap string.
    pub fn to_json_string(&self) -> String {
        encode_to_string(self)
    }

    /// Convert `SourceMap` to vlq sourcemap string, optionally preserving `sourcesContent` when all entries are `None`.
    pub fn to_json_string_with_null_contents(&self, keep_null_contents: bool) -> String {
        crate::encode::encode_to_string_with_null_contents(self, keep_null_contents)
    }

    /// Convert `SourceMap` to vlq sourcemap data url.
    pub fn to_data_url(&self) -> String {
        let base_64_str = base64_simd::STANDARD.encode_to_string(self.to_json_string().as_bytes());
        format!("data:application/json;charset=utf-8;base64,{base_64_str}")
    }

    /// Detach this `SourceMap` from its input buffer by allocating owned
    /// copies of any borrowed strings. Use this when the resulting map
    /// needs to outlive the JSON input it was parsed from, or when the
    /// borrow lifetime from a concat builder is shorter than where you
    /// want to use the result.
    ///
    /// `Cow::Owned` entries are moved without copying; `Cow::Borrowed`
    /// entries allocate.
    pub fn into_owned(self) -> SourceMap<'static> {
        SourceMap {
            file: self.file.map(|c| Cow::Owned(c.into_owned())),
            names: self.names.into_iter().map(|c| Cow::Owned(c.into_owned())).collect(),
            source_root: self.source_root.map(|c| Cow::Owned(c.into_owned())),
            sources: self.sources.into_iter().map(|c| Cow::Owned(c.into_owned())).collect(),
            source_contents: self
                .source_contents
                .into_iter()
                .map(|opt| opt.map(|c| Cow::Owned(c.into_owned())))
                .collect(),
            tokens: self.tokens,
            token_chunks: self.token_chunks,
            ignore_list: self.ignore_list,
            debug_id: self.debug_id.map(|c| Cow::Owned(c.into_owned())),
        }
    }

    /// Decompose this `SourceMap` into its constituent owned parts.
    ///
    /// Useful for downstream code that wants to consume the map (e.g.
    /// transform tokens, swap in a different `file` field) without
    /// re-cloning every name/source string via accessors. Pair with
    /// [`SourceMap::from_parts`] (or `From`) to reassemble.
    pub fn into_parts(self) -> SourceMapParts<'a> {
        SourceMapParts {
            file: self.file,
            names: self.names,
            source_root: self.source_root,
            sources: self.sources,
            source_contents: self.source_contents,
            tokens: self.tokens,
            token_chunks: self.token_chunks,
            ignore_list: self.ignore_list,
            debug_id: self.debug_id,
        }
    }

    /// Reassemble a `SourceMap` from its parts. See [`SourceMap::into_parts`].
    pub fn from_parts(parts: SourceMapParts<'a>) -> Self {
        Self {
            file: parts.file,
            names: parts.names,
            source_root: parts.source_root,
            sources: parts.sources,
            source_contents: parts.source_contents,
            tokens: parts.tokens,
            token_chunks: parts.token_chunks,
            ignore_list: parts.ignore_list,
            debug_id: parts.debug_id,
        }
    }

    pub fn get_file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub fn set_file(&mut self, file: &str) {
        self.file = Some(Cow::Owned(file.to_owned()));
    }

    pub fn get_source_root(&self) -> Option<&str> {
        self.source_root.as_deref()
    }

    pub fn get_ignore_list(&self) -> Option<&[u32]> {
        self.ignore_list.as_deref()
    }

    /// Set `ignoreList`.
    pub fn set_ignore_list(&mut self, ignore_list: Vec<u32>) {
        self.ignore_list = Some(ignore_list);
    }

    pub fn set_debug_id(&mut self, debug_id: &str) {
        self.debug_id = Some(Cow::Owned(debug_id.to_owned()));
    }

    pub fn get_debug_id(&self) -> Option<&str> {
        self.debug_id.as_deref()
    }

    pub fn get_names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.names.iter().map(AsRef::as_ref)
    }

    /// Adjust `sources`.
    pub fn set_sources<S: AsRef<str>, I: IntoIterator<Item = S>>(&mut self, sources: I) {
        self.sources = sources.into_iter().map(|s| Cow::Owned(s.as_ref().to_owned())).collect();
    }

    pub fn get_sources(&self) -> impl ExactSizeIterator<Item = &str> {
        self.sources.iter().map(AsRef::as_ref)
    }

    /// Adjust `source_content`.
    pub fn set_source_contents(&mut self, source_contents: Vec<Option<&str>>) {
        self.source_contents =
            source_contents.into_iter().map(|v| v.map(|s| Cow::Owned(s.to_owned()))).collect();
    }

    pub fn get_source_contents(&self) -> impl ExactSizeIterator<Item = Option<&str>> {
        self.source_contents.iter().map(Option::as_deref)
    }

    /// Filters `source_contents` in place without cloning kept entries.
    pub fn redact_source_contents<E>(
        &mut self,
        mut keep: impl FnMut(&str, &str) -> std::result::Result<bool, E>,
    ) -> std::result::Result<bool, E> {
        if self.source_contents.len() != self.sources.len() || self.source_root.is_some() {
            return Ok(false);
        }
        for (source, content) in self.sources.iter().zip(self.source_contents.iter_mut()) {
            if let Some(text) = content.as_deref() {
                if !keep(source.as_ref(), text)? {
                    *content = None;
                }
            }
        }
        Ok(true)
    }

    pub fn get_token(&self, index: u32) -> Option<Token> {
        self.tokens.get(index as usize).copied()
    }

    pub fn get_source_view_token(&self, index: u32) -> Option<SourceViewToken<'_, 'a>> {
        self.get_token(index).map(|token| SourceViewToken::new(token, self))
    }

    /// Get raw tokens.
    pub fn get_tokens(&self) -> impl ExactSizeIterator<Item = Token> {
        self.tokens.iter().copied()
    }

    /// Get source view tokens. See [`SourceViewToken`] for more information.
    pub fn get_source_view_tokens(&self) -> impl ExactSizeIterator<Item = SourceViewToken<'_, 'a>> {
        self.tokens.iter().map(|&token| SourceViewToken::new(token, self))
    }

    pub fn get_name(&self, id: u32) -> Option<&str> {
        self.names.get(id as usize).map(AsRef::as_ref)
    }

    pub fn get_source(&self, id: u32) -> Option<&str> {
        self.sources.get(id as usize).map(AsRef::as_ref)
    }

    pub fn get_source_content(&self, id: u32) -> Option<&str> {
        self.source_contents.get(id as usize).and_then(Option::as_deref)
    }

    pub fn get_source_and_content(&self, id: u32) -> Option<(&str, &str)> {
        Some((self.get_source(id)?, self.get_source_content(id)?))
    }

    /// Generate a lookup table, it will be used at `lookup_token` or `lookup_source_view_token`.
    pub fn generate_lookup_table(&self) -> Vec<LineLookupTable<'_>> {
        // The dst line/dst col always has increasing order.
        let Some(last_token) = self.tokens.last() else { return vec![] };
        let mut table = vec![&self.tokens[..0]; last_token.dst_line as usize + 1];
        let mut start = 0;
        let mut line = 0;
        for (idx, token) in self.tokens.iter().enumerate() {
            if token.dst_line != line {
                table[line as usize] = &self.tokens[start..idx];
                start = idx;
                line = token.dst_line;
            }
        }
        table[line as usize] = &self.tokens[start..];
        table
    }

    /// Lookup a token by line and column, it will used at remapping.
    #[inline]
    pub fn lookup_token(
        &self,
        lookup_table: &[LineLookupTable],
        line: u32,
        col: u32,
    ) -> Option<Token> {
        // Every token in `lookup_table[line]` has `dst_line == line` (see `generate_lookup_table`),
        // so the line component of the key is constant across the slice. Binary searching on
        // `dst_col` alone avoids building and lexicographically comparing a `(dst_line, dst_col)`
        // tuple on every probe — measurably faster on this remapping hot path.
        greatest_lower_bound(lookup_table.get(line as usize)?, &col, |token| token.dst_col).copied()
    }

    /// Lookup a token by line and column, it will used at remapping. See `SourceViewToken`.
    pub fn lookup_source_view_token(
        &self,
        lookup_table: &[LineLookupTable],
        line: u32,
        col: u32,
    ) -> Option<SourceViewToken<'_, 'a>> {
        self.lookup_token(lookup_table, line, col).map(|token| SourceViewToken::new(token, self))
    }

    /// Like [`lookup_token`](Self::lookup_token), but clamps to the line's first token when `col`
    /// falls *before* it, instead of returning `None`.
    ///
    /// `lookup_token` returns the greatest token with destination `<= (line, col)`, so a query
    /// before a line's first token yields `None`. That is correct for a plain position lookup, but
    /// it drops mappings when *composing* sourcemaps — e.g. remapping a token whose column lands
    /// before the previous map's first token on that line (a line indented past column 0). Clamping
    /// to the nearest token keeps the mapping, matching Rollup's `collapseSourcemaps` `traceSegment`.
    /// An empty line (or a line past the end of the table) still returns `None`.
    pub fn lookup_token_approx(
        &self,
        lookup_table: &[LineLookupTable],
        line: u32,
        col: u32,
    ) -> Option<Token> {
        let line_tokens = lookup_table.get(line as usize)?;
        // Search by `dst_col` alone for the same reason as `lookup_token`.
        greatest_lower_bound(line_tokens, &col, |token| token.dst_col)
            .or_else(|| line_tokens.first())
            .copied()
    }

    /// The [`SourceViewToken`] counterpart of [`lookup_token_approx`](Self::lookup_token_approx).
    pub fn lookup_source_view_token_approx(
        &self,
        lookup_table: &[LineLookupTable],
        line: u32,
        col: u32,
    ) -> Option<SourceViewToken<'_, 'a>> {
        self.lookup_token_approx(lookup_table, line, col)
            .map(|token| SourceViewToken::new(token, self))
    }
}

/// Owned destructured parts of a [`SourceMap`].
///
/// Returned by [`SourceMap::into_parts`] for downstream code that wants to
/// take ownership of the internal `Vec<Cow<'_, str>>` storage without going
/// through accessors (which only return `&str` and force a clone to take
/// ownership).
#[derive(Debug, Clone, Default)]
pub struct SourceMapParts<'a> {
    pub file: Option<Cow<'a, str>>,
    pub names: Vec<Cow<'a, str>>,
    pub source_root: Option<Cow<'a, str>>,
    pub sources: Vec<Cow<'a, str>>,
    pub source_contents: Vec<Option<Cow<'a, str>>>,
    pub tokens: Box<[Token]>,
    pub token_chunks: Option<Vec<TokenChunk>>,
    pub ignore_list: Option<Vec<u32>>,
    pub debug_id: Option<Cow<'a, str>>,
}

impl<'a> From<SourceMapParts<'a>> for SourceMap<'a> {
    fn from(parts: SourceMapParts<'a>) -> Self {
        SourceMap::from_parts(parts)
    }
}

type LineLookupTable<'a> = &'a [Token];

fn greatest_lower_bound<'a, T, K: Ord, F: Fn(&'a T) -> K>(
    slice: &'a [T],
    key: &K,
    map: F,
) -> Option<&'a T> {
    let mut idx = match slice.binary_search_by_key(key, &map) {
        Ok(index) => index,
        Err(index) => {
            // If there is no match, then we know for certain that the index is where we should
            // insert a new token, and that the token directly before is the greatest lower bound.
            return slice.get(index.checked_sub(1)?);
        }
    };

    // If we get an exact match, then we need to continue looking at previous tokens to see if
    // they also match. We use a linear search because the number of exact matches is generally
    // very small, and almost certainly smaller than the number of tokens before the index.
    while idx > 0 && map(&slice[idx - 1]) == *key {
        idx -= 1;
    }
    slice.get(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_token() {
        let input = r#"{
            "version": 3,
            "sources": ["coolstuff.js"],
            "sourceRoot": "x",
            "names": ["x","alert"],
            "mappings": "AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM"
        }"#;
        let sm = SourceMap::from_json_string(input).unwrap();
        let lookup_table = sm.generate_lookup_table();
        assert_eq!(
            sm.lookup_source_view_token(&lookup_table, 0, 0).unwrap().to_tuple(),
            (Some("coolstuff.js"), 0, 0, None)
        );
        assert_eq!(
            sm.lookup_source_view_token(&lookup_table, 0, 3).unwrap().to_tuple(),
            (Some("coolstuff.js"), 0, 4, Some("x"))
        );
        assert_eq!(
            sm.lookup_source_view_token(&lookup_table, 0, 24).unwrap().to_tuple(),
            (Some("coolstuff.js"), 2, 8, None)
        );

        // Lines continue out to infinity
        assert_eq!(
            sm.lookup_source_view_token(&lookup_table, 0, 1000).unwrap().to_tuple(),
            (Some("coolstuff.js"), 2, 8, None)
        );

        assert!(sm.lookup_source_view_token(&lookup_table, 1000, 0).is_none());
        // Plain `lookup_token` (not the source-view wrapper) on the same table.
        assert!(sm.lookup_token(&lookup_table, 0, 0).is_some());
    }

    #[test]
    fn source_view_token() {
        let sm = SourceMap::new(
            None,
            vec![Cow::Borrowed("foo")],
            None,
            vec![Cow::Borrowed("foo.js")],
            vec![],
            vec![Token::new(1, 1, 1, 1, Some(0), Some(0))].into_boxed_slice(),
            None,
        );
        let mut source_view_tokens = sm.get_source_view_tokens();
        assert_eq!(
            source_view_tokens.next().unwrap().to_tuple(),
            (Some("foo.js"), 1, 1, Some("foo"))
        );
        // Indexed source-view token accessor, including out-of-range.
        assert!(sm.get_source_view_token(0).is_some());
        assert!(sm.get_source_view_token(99).is_none());
    }

    #[test]
    fn mut_sourcemap() {
        let mut sm = SourceMap::default();
        sm.set_file("index.js");
        sm.set_sources(vec!["foo.js"]);
        sm.set_source_contents(vec![Some("foo")]);

        assert_eq!(sm.get_file(), Some("index.js"));
        assert_eq!(sm.get_source(0), Some("foo.js"));
        assert_eq!(sm.get_source_content(0), Some("foo"));
    }

    #[test]
    fn from_json_value() {
        // `SourceMap::from_json` consumes an already-deserialized `JSONSourceMap`,
        // a different path from the borrowed `from_json_string`.
        let json = SourceMap::from_json_string(
            r#"{"version":3,"sources":["a.js"],"names":[],"mappings":"AAAA"}"#,
        )
        .unwrap()
        .to_json();
        let sm = SourceMap::from_json(json).unwrap();
        assert_eq!(sm.get_source(0), Some("a.js"));
    }

    #[test]
    fn to_data_url() {
        let sm =
            SourceMap::from_json_string(r#"{"version":3,"sources":[],"names":[],"mappings":""}"#)
                .unwrap();
        assert!(sm.to_data_url().starts_with("data:application/json;charset=utf-8;base64,"));
    }

    #[test]
    fn source_and_content() {
        let sm = SourceMap::from_json_string(
            r#"{"version":3,"sources":["a.js"],"sourcesContent":["CONTENT"],"names":[],"mappings":"AAAA"}"#,
        )
        .unwrap();
        assert_eq!(sm.get_source_and_content(0), Some(("a.js", "CONTENT")));
        // Out-of-range source id short-circuits before reading content.
        assert_eq!(sm.get_source_and_content(99), None);

        // Source present but its content is absent: the second `?` returns `None`.
        let no_content = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js")],
            vec![],
            vec![].into_boxed_slice(),
            None,
        );
        assert_eq!(no_content.get_source(0), Some("a.js"));
        assert_eq!(no_content.get_source_and_content(0), None);
    }

    #[test]
    fn parts_roundtrip() {
        let sm = SourceMap::from_json_string(
            r#"{"version":3,"file":"f.js","sources":["a.js"],"names":["n"],"mappings":"AAAAA"}"#,
        )
        .unwrap();
        let parts = sm.into_parts();
        assert_eq!(parts.file.as_deref(), Some("f.js"));
        // Reassemble via `from_parts` and via the `From<SourceMapParts>` impl.
        let rebuilt = SourceMap::from_parts(parts);
        assert_eq!(rebuilt.get_file(), Some("f.js"));
        let from_parts: SourceMap = rebuilt.into_parts().into();
        assert_eq!(from_parts.get_source(0), Some("a.js"));
    }

    #[test]
    fn empty_lookup_table() {
        // No tokens => empty table, and lookups return `None`.
        let sm = SourceMap::default();
        assert!(sm.generate_lookup_table().is_empty());
        assert!(sm.lookup_token(&[], 0, 0).is_none());
    }

    #[test]
    fn lookup_before_first_token() {
        // The only token starts at column 5; looking up column 0 falls before
        // it, exercising the `checked_sub(1)` underflow guard in
        // `greatest_lower_bound`.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js")],
            vec![],
            vec![Token::new(0, 5, 0, 0, Some(0), None)].into_boxed_slice(),
            None,
        );
        let table = sm.generate_lookup_table();
        assert!(sm.lookup_token(&table, 0, 0).is_none());
    }

    #[test]
    fn lookup_with_duplicate_columns() {
        // Several tokens share the same generated position (line 0, col 7). A
        // lookup there finds an exact match, then walks back over the equal-key
        // run in `greatest_lower_bound` and returns the *first* duplicate.
        let mut tokens = vec![Token::new(0, 0, 9, 9, Some(0), None)];
        for src_line in 1..=5 {
            tokens.push(Token::new(0, 7, src_line, 0, Some(0), None));
        }
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js")],
            vec![],
            tokens.into_boxed_slice(),
            None,
        );
        let table = sm.generate_lookup_table();
        // The earliest token at column 7 wins (src_line 1).
        assert_eq!(sm.lookup_token(&table, 0, 7).unwrap().get_src_line(), 1);
    }
}
