use std::fmt::Write;

use crate::SourceMap;

/// The `SourcemapVisualizer` is a helper for sourcemap testing.
/// It print the mapping of original content and final content tokens.
pub struct SourcemapVisualizer<'a, 'sm> {
    code: &'a str,
    sourcemap: &'a SourceMap<'sm>,
}

impl<'a, 'sm> SourcemapVisualizer<'a, 'sm> {
    pub fn new(code: &'a str, sourcemap: &'a SourceMap<'sm>) -> Self {
        Self { code, sourcemap }
    }

    pub fn get_url(&self) -> String {
        let result = self.sourcemap.to_json_string();
        let s = format!("{}\0{}{}\0{}", self.code.len(), self.code, result.len(), result);
        let hash = base64_simd::STANDARD.encode_to_string(s);
        format!("https://evanw.github.io/source-map-visualization/#{hash}")
    }

    pub fn get_text(&self) -> String {
        let source_contents = &self.sourcemap.source_contents;
        if source_contents.is_empty() {
            return "[no source contents]\n".to_owned();
        }

        // Build a 1:1 map: index N in the result corresponds to source_id N.
        // `None` entries are preserved so indexing by source_id stays correct
        // even when some sources have no content (the previous filter_map
        // dropped them, which misaligned all later indices).
        let source_contents_lines_map: Vec<Option<Vec<Vec<u16>>>> = source_contents
            .iter()
            .map(|content| content.as_deref().map(Self::generate_line_utf16_tables))
            .collect();

        let output_lines = Self::generate_line_utf16_tables(self.code);

        let tokens = &self.sourcemap.tokens;

        let mut s = String::new();
        let mut last_source = None;
        for (i, t) in tokens.iter().enumerate() {
            let Some(source_id) = t.get_source_id() else {
                continue;
            };
            let Some(source) = self.sourcemap.get_source(source_id) else { continue };
            let Some(source_lines) =
                source_contents_lines_map.get(source_id as usize).and_then(Option::as_ref)
            else {
                // No content for this source; skip rather than panic.
                continue;
            };

            // Print source
            if last_source != Some(source) {
                writeln!(s, "- {source}").unwrap();
                last_source = Some(source);
            }

            // validate token position
            let dst_invalid = t.dst_line as usize >= output_lines.len()
                || (t.dst_col as usize) >= output_lines[t.dst_line as usize].len();
            let src_invalid = t.src_line as usize >= source_lines.len()
                || (t.src_col as usize) >= source_lines[t.src_line as usize].len();
            if dst_invalid || src_invalid {
                writeln!(
                    s,
                    "({}:{}){} --> ({}:{}){}",
                    t.src_line,
                    t.src_col,
                    if src_invalid { " [invalid]" } else { "" },
                    t.dst_line,
                    t.dst_col,
                    if dst_invalid { " [invalid]" } else { "" },
                )
                .unwrap();
                continue;
            }

            // find next dst column or EOL
            let dst_end_col = match tokens.get(i + 1) {
                Some(t2) if t2.dst_line == t.dst_line => t2.dst_col,
                _ => output_lines[t.dst_line as usize].len() as u32,
            };

            // find next src column or EOL
            let src_end_col = tokens[i + 1..]
                .iter()
                .take_while(|t2| t2.get_source_id() == Some(source_id) && t2.src_line == t.src_line)
                .find(|t2| t2.src_col > t.src_col)
                .map_or(source_lines[t.src_line as usize].len() as u32, |t2| t2.src_col);

            writeln!(
                s,
                "({}:{}) {:?} --> ({}:{}) {:?}",
                t.src_line,
                t.src_col,
                Self::str_slice_by_token(source_lines, t.src_line, t.src_col, src_end_col),
                t.dst_line,
                t.dst_col,
                Self::str_slice_by_token(&output_lines, t.dst_line, t.dst_col, dst_end_col)
            )
            .unwrap();
        }

        s
    }

    fn generate_line_utf16_tables(content: &str) -> Vec<Vec<u16>> {
        let mut tables = vec![];
        let mut line_byte_offset = 0;
        let bytes = content.as_bytes();
        for (i, ch) in content.char_indices() {
            if matches!(ch, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
                // Keep CRLF together. The next byte is ASCII even after multibyte UTF-8.
                if ch == '\r' && bytes.get(i + 1) == Some(&b'\n') {
                    continue;
                }
                let end = i + ch.len_utf8();
                tables.push(content[line_byte_offset..end].encode_utf16().collect());
                line_byte_offset = end;
            }
        }
        tables.push(content[line_byte_offset..].encode_utf16().collect());
        tables
    }

    fn str_slice_by_token(buff: &[Vec<u16>], line: u32, start: u32, end: u32) -> String {
        let start = start as usize;
        let end = end as usize;
        let s = &buff[line as usize];
        // A mapping can split a surrogate pair, so render incomplete characters lossily.
        String::from_utf16_lossy(&s[start.min(end).min(s.len())..start.max(end).min(s.len())])
            .replace('\r', "")
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::Token;

    #[test]
    fn get_url() {
        let sm =
            SourceMap::from_json_string(r#"{"version":3,"sources":[],"names":[],"mappings":""}"#)
                .unwrap();
        let url = SourcemapVisualizer::new("code", &sm).get_url();
        assert!(url.starts_with("https://evanw.github.io/source-map-visualization/#"));
    }

    #[test]
    fn no_source_contents() {
        // Sources present but no `sourcesContent` at all.
        let sm = SourceMap::from_json_string(
            r#"{"version":3,"sources":["a.js"],"names":[],"mappings":"AAAA"}"#,
        )
        .unwrap();
        assert_eq!(SourcemapVisualizer::new("a", &sm).get_text(), "[no source contents]\n");
    }

    #[test]
    fn skips_tokens_without_resolvable_source() {
        // First token has no source id, second points at a source whose content
        // is `None`; both are skipped, and only the third (valid) token prints.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js"), Cow::Borrowed("b.js")],
            vec![Some(Cow::Borrowed("hello\n")), None],
            vec![
                Token::new(0, 0, 0, 0, None, None),
                Token::new(0, 1, 0, 0, Some(1), None),
                Token::new(0, 2, 0, 0, Some(0), None),
            ]
            .into_boxed_slice(),
            None,
        );
        let text = SourcemapVisualizer::new("hello\n", &sm).get_text();
        assert!(text.contains("- a.js"), "{text}");
        assert!(!text.contains("- b.js"), "{text}");
    }

    #[test]
    fn handles_crlf_line_endings() {
        // CRLF source content exercises the `\r\n` peek branch in the line table.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js")],
            vec![Some(Cow::Borrowed("aa\r\nbb\r\n"))],
            vec![Token::new(0, 0, 0, 0, Some(0), None), Token::new(1, 0, 1, 0, Some(0), None)]
                .into_boxed_slice(),
            None,
        );
        let text = SourcemapVisualizer::new("aa\r\nbb\r\n", &sm).get_text();
        assert!(text.contains("- a.js"), "{text}");
    }

    #[test]
    fn handles_unicode_line_separators() {
        for separator in ['\u{2028}', '\u{2029}'] {
            let code = format!("é{separator}😀");
            let sm = SourceMap::new(
                None,
                vec![],
                None,
                vec!["a.js".into()],
                vec![Some(code.as_str().into())],
                vec![Token::new(0, 0, 0, 0, Some(0), None), Token::new(1, 0, 1, 0, Some(0), None)]
                    .into_boxed_slice(),
                None,
            );
            let first_line = format!("é{separator}");
            assert_eq!(
                SourcemapVisualizer::new(&code, &sm).get_text(),
                format!(
                    "- a.js\n(0:0) {first_line:?} --> (0:0) {first_line:?}\n(1:0) \"😀\" --> (1:0) \"😀\"\n"
                )
            );
        }
    }

    #[test]
    fn handles_columns_inside_surrogate_pairs() {
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into()],
            vec![Some("😀x".into())],
            vec![Token::new(0, 0, 0, 0, Some(0), None), Token::new(0, 1, 0, 1, Some(0), None)]
                .into_boxed_slice(),
            None,
        );
        assert_eq!(
            SourcemapVisualizer::new("😀x", &sm).get_text(),
            "- a.js\n(0:0) \"�\" --> (0:0) \"�\"\n(0:1) \"�x\" --> (0:1) \"�x\"\n"
        );
    }

    #[test]
    fn skips_token_with_out_of_range_source() {
        // A token references a source id past the end of `sources`; the
        // visualizer skips it via the `get_source` guard rather than panicking.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![Cow::Borrowed("a.js")],
            vec![Some(Cow::Borrowed("aa\n"))],
            vec![Token::new(0, 0, 0, 0, Some(5), None)].into_boxed_slice(),
            None,
        );
        assert_eq!(SourcemapVisualizer::new("aa\n", &sm).get_text(), "");
    }
}
