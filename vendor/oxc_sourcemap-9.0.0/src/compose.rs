use std::borrow::Cow;

use rustc_hash::FxHashMap;

use crate::{SourceMap, Token};

impl SourceMap<'_> {
    /// Compose this source map with the source map for its input.
    ///
    /// Given a map from generated code to intermediate code (`self`) and a map
    /// from that intermediate code to original code (`input`), returns a map
    /// from the generated code to the original code.
    ///
    /// A transformation map may contain at most one source because all of its
    /// original positions must use the input map's generated coordinate space.
    /// The input map may contain any number of original sources.
    ///
    /// Generated positions, `file`, and `debugId` come from `self`. Original
    /// positions and source metadata come from `input`. An input name takes
    /// precedence over a name from `self` at the same mapping.
    ///
    /// # Panics
    ///
    /// Panics when `self` contains more than one source.
    pub fn compose<'input>(self, input: SourceMap<'input>) -> SourceMap<'input> {
        let generated = self.into_parts();
        assert!(
            generated.sources.len() <= 1,
            "Cannot compose a transformation map with {} sources",
            generated.sources.len()
        );

        let (tokens, fallback_names) = {
            let lookup_table = input.generate_lookup_table();
            let mut name_ids = FxHashMap::default();
            for (name_id, name) in input.names.iter().enumerate() {
                name_ids.entry(name.as_ref()).or_insert(name_id as u32);
            }

            let mut fallback_names = Vec::new();
            let mut intern_generated_name = |name_id: Option<u32>| {
                let name = generated.names.get(name_id? as usize)?.as_ref();
                Some(*name_ids.entry(name).or_insert_with(|| {
                    let name_id = (input.names.len() + fallback_names.len()) as u32;
                    fallback_names.push(name.to_owned());
                    name_id
                }))
            };

            let tokens = generated
                .tokens
                .iter()
                .map(|token| {
                    let original = token.get_source_id().and_then(|_| {
                        input.lookup_token_approx(
                            &lookup_table,
                            token.get_src_line(),
                            token.get_src_col(),
                        )
                    });
                    let Some(original) = original.filter(|t| t.get_source_id().is_some()) else {
                        return unmapped_token(token);
                    };

                    Token::new(
                        token.get_dst_line(),
                        token.get_dst_col(),
                        original.get_src_line(),
                        original.get_src_col(),
                        original.get_source_id(),
                        original
                            .get_name_id()
                            .or_else(|| intern_generated_name(token.get_name_id())),
                    )
                })
                .collect();

            (tokens, fallback_names)
        };

        let mut result = input.into_parts();
        result.file = generated.file.map(|file| Cow::Owned(file.into_owned()));
        result.names.extend(fallback_names.into_iter().map(Cow::Owned));
        result.tokens = tokens;
        result.token_chunks = None;
        result.debug_id = generated.debug_id.map(|debug_id| Cow::Owned(debug_id.into_owned()));
        SourceMap::from_parts(result)
    }
}

fn unmapped_token(token: &Token) -> Token {
    Token::new(token.get_dst_line(), token.get_dst_col(), 0, 0, None, None)
}
