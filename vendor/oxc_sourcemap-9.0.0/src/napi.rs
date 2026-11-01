use napi_derive::napi;

// Source map output using the standard ignoreList field.
// See <https://tc39.es/ecma426/#sec-source-map-format>.
//
// ```
// export interface ExistingRawSourceMap {
//  file?: string;
//  mappings: string;
//  names: string[];
//  sourceRoot?: string;
//  sources: string[];
//  sourcesContent?: string[];
//  version: number;
//  ignoreList?: number[];
// }
// ```
#[napi(object)]
pub struct SourceMap {
    pub file: Option<String>,
    pub mappings: String,
    pub names: Vec<String>,
    pub source_root: Option<String>,
    pub sources: Vec<String>,
    pub sources_content: Option<Vec<String>>,
    pub version: u8,
    #[napi(js_name = "ignoreList")]
    pub ignore_list: Option<Vec<u32>>,
}

impl From<crate::SourceMap<'_>> for SourceMap {
    fn from(source_map: crate::SourceMap<'_>) -> Self {
        let json = source_map.to_json();
        Self {
            file: json.file,
            mappings: json.mappings,
            names: json.names,
            source_root: json.source_root,
            sources: json.sources,
            sources_content: json
                .sources_content
                .map(|content| content.into_iter().map(Option::unwrap_or_default).collect()),
            version: 3,
            ignore_list: json.ignore_list,
        }
    }
}

impl From<crate::OwnedSourceMap> for SourceMap {
    #[inline]
    fn from(source_map: crate::OwnedSourceMap) -> Self {
        Self::from(source_map.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceMap;

    #[test]
    fn from_source_map() {
        let mut inner = crate::SourceMap::new(
            Some("out.js".into()),
            vec!["n0".into()],
            Some("root".into()),
            vec!["a.js".into()],
            vec![Some("content".into())],
            vec![].into_boxed_slice(),
            None,
        );
        inner.set_ignore_list(vec![0]);

        let napi: SourceMap = inner.into();
        assert_eq!(napi.version, 3);
        assert_eq!(napi.file.as_deref(), Some("out.js"));
        assert_eq!(napi.source_root.as_deref(), Some("root"));
        assert_eq!(napi.names, vec!["n0".to_string()]);
        assert_eq!(napi.sources, vec!["a.js".to_string()]);
        assert_eq!(napi.sources_content, Some(vec!["content".to_string()]));
        assert_eq!(napi.ignore_list, Some(vec![0]));
    }

    #[test]
    fn null_source_content_becomes_empty_string() {
        // `sourcesContent` entries that are `null` map to an empty string,
        // because the napi shape uses `Vec<String>` (no inner `Option`).
        let inner = crate::SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into(), "b.js".into()],
            vec![Some("x".into()), None],
            vec![].into_boxed_slice(),
            None,
        );
        let napi: SourceMap = inner.into();
        assert_eq!(napi.sources_content, Some(vec!["x".to_string(), String::new()]));
    }

    #[test]
    fn from_owned_source_map() {
        let owned = crate::OwnedSourceMap::from_json_string(
            r#"{"version":3,"sources":["a.js"],"mappings":"","x_google_ignoreList":[0]}"#,
        )
        .unwrap();
        let napi: SourceMap = owned.into();
        assert_eq!(napi.version, 3);
        assert_eq!(napi.sources, ["a.js"]);
        assert_eq!(napi.sources_content, None);
        assert_eq!(napi.ignore_list, Some(vec![0]));
    }
}
