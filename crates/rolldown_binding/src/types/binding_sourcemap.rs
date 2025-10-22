use napi::Either;
use rolldown_error::BuildDiagnostic;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<JSONSourceMap>` impl (line 61) used in hook outputs
// - Receive from JS: `TryFrom<BindingSourcemap>` impl (line 10) used in JsOutputChunk
#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingSourcemap {
  pub inner: Either<String, BindingJsonSourcemap>,
}

impl TryFrom<BindingSourcemap> for rolldown_sourcemap::SourceMap {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingSourcemap) -> Result<Self, Self::Error> {
    match value.inner {
      Either::A(s) => rolldown_sourcemap::SourceMap::from_json_string(&s)
        .map_err(|e| anyhow::anyhow!("Convert string sourcemap error: {e:?}").into()),
      Either::B(v) => v.try_into(),
    }
  }
}

// This struct is used to both pass to JS and receive from JS:
// - Part of BindingSourcemap Either type, used in both directions
// - Pass to JS: Created in From<JSONSourceMap> impl (line 64)
// - Receive from JS: Converted in TryFrom<BindingSourcemap> impl (line 16)
#[derive(Debug, Default)]
#[napi_derive::napi(object)]
pub struct BindingJsonSourcemap {
  pub file: Option<String>,
  pub mappings: Option<String>,
  pub source_root: Option<String>,
  pub sources: Option<Vec<Option<String>>>,
  pub sources_content: Option<Vec<Option<String>>>,
  pub names: Option<Vec<String>>,
  pub debug_id: Option<String>,
  #[napi(js_name = "x_google_ignoreList")]
  pub x_google_ignore_list: Option<Vec<u32>>,
}

impl TryFrom<BindingJsonSourcemap> for rolldown_sourcemap::SourceMap {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingJsonSourcemap) -> Result<Self, Self::Error> {
    let map = rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
      version: 3,
      file: value.file,
      mappings: value.mappings.unwrap_or_default(),
      source_root: value.source_root,
      sources: value
        .sources
        .unwrap_or_default()
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect(),
      sources_content: value.sources_content,
      names: value.names.unwrap_or_default(),
      debug_id: value.debug_id,
      x_google_ignore_list: value.x_google_ignore_list,
    })
    .map_err(|e| anyhow::format_err!("Convert json sourcemap error: {e:?}"))?;
    Ok(map)
  }
}

impl From<rolldown_sourcemap::JSONSourceMap> for BindingSourcemap {
  fn from(value: rolldown_sourcemap::JSONSourceMap) -> Self {
    Self {
      inner: Either::B(BindingJsonSourcemap {
        file: value.file,
        mappings: Some(value.mappings),
        source_root: value.source_root,
        sources: Some(value.sources.into_iter().map(Some).collect()),
        sources_content: value.sources_content,
        names: Some(value.names),
        debug_id: value.debug_id,
        x_google_ignore_list: value.x_google_ignore_list,
      }),
    }
  }
}
