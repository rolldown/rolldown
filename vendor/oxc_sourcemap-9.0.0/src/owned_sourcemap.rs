use crate::{
    JSONSourceMap, SourceMap, SourceViewToken, Token, error::Result, sourcemap::SourceMapParts,
};

/// An owned, lifetime-free source map.
///
/// `OwnedSourceMap` is a thin wrapper around `SourceMap<'static>`. It exists
/// so that downstream code that stores sourcemaps as long-lived struct fields
/// can write `Option<OwnedSourceMap>` instead of `Option<SourceMap<'static>>`,
/// keeping the `'static` lifetime annotation out of the public type names.
///
/// Internally this is just `SourceMap<'static>` — same allocation model, same
/// methods. For the zero-copy-parse case, prefer the lifetime-parameterized
/// [`SourceMap<'a>`] directly.
///
/// ```
/// use oxc_sourcemap::{OwnedSourceMap, SourceMap, SourceMapBuilder};
///
/// // From a builder (always owned):
/// let owned: OwnedSourceMap = SourceMapBuilder::default().into_owned_sourcemap();
///
/// // From a JSON string (zero-copy parse, then detach):
/// let json = r#"{"version":3,"sources":[],"names":[],"mappings":""}"#;
/// let owned = OwnedSourceMap::from_json_string(json).unwrap();
///
/// // Use the same accessors as SourceMap<'_>:
/// let _file: Option<&str> = owned.get_file();
/// let _names: Vec<&str> = owned.get_names().collect();
///
/// // Drop into the underlying `&SourceMap<'_>` when needed (e.g. for
/// // serialization, lookup tables, concat builders):
/// let _inner: &SourceMap<'_> = owned.as_source_map();
/// let _json_str = owned.to_json_string();
/// ```
#[derive(Debug, Clone, Default)]
pub struct OwnedSourceMap {
    inner: SourceMap<'static>,
}

impl OwnedSourceMap {
    /// Wrap an already-`'static` `SourceMap`.
    #[inline]
    pub fn new(inner: SourceMap<'static>) -> Self {
        Self { inner }
    }

    /// Borrow as `&SourceMap<'_>` so generic code that works on `SourceMap`
    /// (lookup tables, concat builder, encoder, visualizer) can consume an
    /// `OwnedSourceMap` without an explicit conversion.
    #[inline]
    pub fn as_source_map(&self) -> &SourceMap<'static> {
        &self.inner
    }

    /// Mutable borrow of the inner `SourceMap`. Useful for `set_file` etc.
    #[inline]
    pub fn as_source_map_mut(&mut self) -> &mut SourceMap<'static> {
        &mut self.inner
    }

    /// Unwrap into the inner `SourceMap<'static>`.
    #[inline]
    pub fn into_inner(self) -> SourceMap<'static> {
        self.inner
    }

    // ---------- parse / serialize ----------

    /// Parse a sourcemap from a `JSONSourceMap` (already-deserialized JSON).
    ///
    /// # Errors
    /// Returns VLQ decode errors.
    pub fn from_json(value: JSONSourceMap) -> Result<Self> {
        SourceMap::from_json(value).map(Self::new)
    }

    /// Parse a sourcemap from a JSON string. Uses the zero-copy borrowed-
    /// deserialization path internally, then detaches to `'static` so the
    /// result outlives `value`.
    ///
    /// # Errors
    /// Returns `serde_json` and VLQ decode errors.
    pub fn from_json_string(value: &str) -> Result<Self> {
        SourceMap::from_json_string(value).map(SourceMap::into_owned_sourcemap)
    }

    pub fn to_json(&self) -> JSONSourceMap {
        self.inner.to_json()
    }

    pub fn to_json_string(&self) -> String {
        self.inner.to_json_string()
    }

    pub fn to_data_url(&self) -> String {
        self.inner.to_data_url()
    }

    /// Compose this generated source map with the source map for its input.
    ///
    /// See [`SourceMap::compose`] for composition direction and metadata behavior.
    pub fn compose(self, input: Self) -> Self {
        Self::new(self.inner.compose(input.inner))
    }

    // ---------- accessors (delegated) ----------

    pub fn get_file(&self) -> Option<&str> {
        self.inner.get_file()
    }

    pub fn set_file(&mut self, file: &str) {
        self.inner.set_file(file);
    }

    pub fn get_source_root(&self) -> Option<&str> {
        self.inner.get_source_root()
    }

    pub fn get_ignore_list(&self) -> Option<&[u32]> {
        self.inner.get_ignore_list()
    }

    pub fn set_ignore_list(&mut self, list: Vec<u32>) {
        self.inner.set_ignore_list(list);
    }

    pub fn get_debug_id(&self) -> Option<&str> {
        self.inner.get_debug_id()
    }

    pub fn set_debug_id(&mut self, debug_id: &str) {
        self.inner.set_debug_id(debug_id);
    }

    pub fn get_names(&self) -> impl Iterator<Item = &str> {
        self.inner.get_names()
    }

    pub fn get_sources(&self) -> impl Iterator<Item = &str> {
        self.inner.get_sources()
    }

    pub fn set_sources<S: AsRef<str>, I: IntoIterator<Item = S>>(&mut self, sources: I) {
        self.inner.set_sources(sources);
    }

    pub fn get_source_contents(&self) -> impl Iterator<Item = Option<&str>> {
        self.inner.get_source_contents()
    }

    pub fn set_source_contents(&mut self, contents: Vec<Option<&str>>) {
        self.inner.set_source_contents(contents);
    }

    pub fn get_name(&self, id: u32) -> Option<&str> {
        self.inner.get_name(id)
    }

    pub fn get_source(&self, id: u32) -> Option<&str> {
        self.inner.get_source(id)
    }

    pub fn get_source_content(&self, id: u32) -> Option<&str> {
        self.inner.get_source_content(id)
    }

    pub fn get_source_and_content(&self, id: u32) -> Option<(&str, &str)> {
        self.inner.get_source_and_content(id)
    }

    pub fn get_token(&self, index: u32) -> Option<Token> {
        self.inner.get_token(index)
    }

    pub fn get_tokens(&self) -> impl Iterator<Item = Token> {
        self.inner.get_tokens()
    }

    pub fn get_source_view_token(&self, index: u32) -> Option<SourceViewToken<'_, 'static>> {
        self.inner.get_source_view_token(index)
    }

    pub fn get_source_view_tokens(&self) -> impl Iterator<Item = SourceViewToken<'_, 'static>> {
        self.inner.get_source_view_tokens()
    }

    pub fn generate_lookup_table(&self) -> Vec<&[Token]> {
        self.inner.generate_lookup_table()
    }

    pub fn lookup_token(&self, lookup_table: &[&[Token]], line: u32, col: u32) -> Option<Token> {
        self.inner.lookup_token(lookup_table, line, col)
    }

    pub fn lookup_source_view_token(
        &self,
        lookup_table: &[&[Token]],
        line: u32,
        col: u32,
    ) -> Option<SourceViewToken<'_, 'static>> {
        self.inner.lookup_source_view_token(lookup_table, line, col)
    }

    pub fn lookup_token_approx(
        &self,
        lookup_table: &[&[Token]],
        line: u32,
        col: u32,
    ) -> Option<Token> {
        self.inner.lookup_token_approx(lookup_table, line, col)
    }

    pub fn lookup_source_view_token_approx(
        &self,
        lookup_table: &[&[Token]],
        line: u32,
        col: u32,
    ) -> Option<SourceViewToken<'_, 'static>> {
        self.inner.lookup_source_view_token_approx(lookup_table, line, col)
    }

    // ---------- structural ----------

    /// Take the inner parts; same as `SourceMap::into_parts`.
    pub fn into_parts(self) -> SourceMapParts<'static> {
        self.inner.into_parts()
    }

    /// Build from parts; same as `SourceMap::from_parts`.
    pub fn from_parts(parts: SourceMapParts<'static>) -> Self {
        Self::new(SourceMap::from_parts(parts))
    }
}

impl From<SourceMap<'static>> for OwnedSourceMap {
    #[inline]
    fn from(inner: SourceMap<'static>) -> Self {
        Self::new(inner)
    }
}

impl From<OwnedSourceMap> for SourceMap<'static> {
    #[inline]
    fn from(owned: OwnedSourceMap) -> Self {
        owned.into_inner()
    }
}

impl std::ops::Deref for OwnedSourceMap {
    type Target = SourceMap<'static>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for OwnedSourceMap {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a> SourceMap<'a> {
    /// Materialize an owned sourcemap from `self`. Equivalent to
    /// `OwnedSourceMap::new(self.into_owned())`.
    #[inline]
    pub fn into_owned_sourcemap(self) -> OwnedSourceMap {
        OwnedSourceMap::new(self.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "version": 3,
        "file": "out.js",
        "sourceRoot": "root",
        "names": ["n0"],
        "sources": ["a.js"],
        "sourcesContent": ["AC"],
        "mappings": "AAAAA",
        "ignoreList": [0],
        "debugId": "dbg"
    }"#;

    #[test]
    fn accessors() {
        let sm = OwnedSourceMap::from_json_string(JSON).unwrap();
        assert_eq!(sm.get_file(), Some("out.js"));
        assert_eq!(sm.get_source_root(), Some("root"));
        assert_eq!(sm.get_ignore_list(), Some(&[0][..]));
        assert_eq!(sm.get_debug_id(), Some("dbg"));
        assert_eq!(sm.get_names().collect::<Vec<_>>(), vec!["n0"]);
        assert_eq!(sm.get_sources().collect::<Vec<_>>(), vec!["a.js"]);
        assert_eq!(sm.get_source_contents().collect::<Vec<_>>(), vec![Some("AC")]);
        assert_eq!(sm.get_name(0), Some("n0"));
        assert_eq!(sm.get_source(0), Some("a.js"));
        assert_eq!(sm.get_source_content(0), Some("AC"));
        assert_eq!(sm.get_source_and_content(0), Some(("a.js", "AC")));
        assert!(sm.get_token(0).is_some());
        assert_eq!(sm.get_tokens().count(), 1);
        assert!(sm.get_source_view_token(0).is_some());
        assert_eq!(sm.get_source_view_tokens().count(), 1);
    }

    #[test]
    fn mutators() {
        let mut sm = OwnedSourceMap::from_json_string(JSON).unwrap();
        sm.set_file("new.js");
        assert_eq!(sm.get_file(), Some("new.js"));
        sm.set_sources(vec!["b.js"]);
        assert_eq!(sm.get_source(0), Some("b.js"));
        sm.set_source_contents(vec![Some("new content")]);
        assert_eq!(sm.get_source_content(0), Some("new content"));
        sm.set_ignore_list(vec![]);
        assert_eq!(sm.get_ignore_list(), Some(&[][..]));
        sm.set_debug_id("newdbg");
        assert_eq!(sm.get_debug_id(), Some("newdbg"));
    }

    #[test]
    fn lookup() {
        let sm = OwnedSourceMap::from_json_string(JSON).unwrap();
        let table = sm.generate_lookup_table();
        assert!(sm.lookup_token(&table, 0, 0).is_some());
        assert!(sm.lookup_source_view_token(&table, 0, 0).is_some());
    }

    #[test]
    fn serialize() {
        let sm = OwnedSourceMap::from_json_string(JSON).unwrap();
        assert_eq!(sm.to_json().file.as_deref(), Some("out.js"));
        assert!(sm.to_json_string().contains("\"file\":\"out.js\""));
        assert!(sm.to_data_url().starts_with("data:application/json;charset=utf-8;base64,"));
    }

    #[test]
    fn from_json_value() {
        let json = SourceMap::from_json_string(JSON).unwrap().to_json();
        let sm = OwnedSourceMap::from_json(json).unwrap();
        assert_eq!(sm.get_file(), Some("out.js"));
    }

    #[test]
    fn conversions_and_inner_access() {
        let inner = SourceMap::from_json_string(JSON).unwrap().into_owned();

        // `new` + borrow accessors.
        let owned = OwnedSourceMap::new(inner.clone());
        assert_eq!(owned.as_source_map().get_file(), Some("out.js"));

        // `From<SourceMap>` + mutable inner access.
        let mut from_sm = OwnedSourceMap::from(inner.clone());
        from_sm.as_source_map_mut().set_file("z.js");
        assert_eq!(from_sm.get_file(), Some("z.js"));

        // `Deref` / `DerefMut` to the inner `SourceMap`.
        let deref: &SourceMap<'static> = &owned;
        assert_eq!(deref.get_file(), Some("out.js"));
        let mut owned_mut = owned.clone();
        let deref_mut: &mut SourceMap<'static> = &mut owned_mut;
        deref_mut.set_file("deref.js");
        assert_eq!(owned_mut.get_file(), Some("deref.js"));

        // `into_inner` + `From<OwnedSourceMap>` + `SourceMap::into_owned_sourcemap`.
        let back: SourceMap<'static> = owned.into_inner();
        let round: SourceMap<'static> = OwnedSourceMap::from(back.clone()).into();
        assert_eq!(round.get_file(), Some("out.js"));
        assert_eq!(back.into_owned_sourcemap().get_file(), Some("out.js"));
    }

    #[test]
    fn parts_roundtrip() {
        let owned = OwnedSourceMap::from_json_string(JSON).unwrap();
        let parts = owned.into_parts();
        assert_eq!(parts.file.as_deref(), Some("out.js"));
        assert_eq!(parts.ignore_list.as_deref(), Some(&[0][..]));
        let rebuilt = OwnedSourceMap::from_parts(parts);
        assert_eq!(rebuilt.get_file(), Some("out.js"));
        assert_eq!(rebuilt.get_source(0), Some("a.js"));
        assert_eq!(rebuilt.get_ignore_list(), Some(&[0][..]));
    }
}
