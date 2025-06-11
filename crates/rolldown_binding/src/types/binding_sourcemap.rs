use napi::Either;

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingSourcemap {
  pub inner: Either<String, BindingJsonSourcemap>,
}

impl TryFrom<BindingSourcemap> for rolldown_sourcemap::SourceMap {
  type Error = anyhow::Error;

  fn try_from(value: BindingSourcemap) -> Result<Self, Self::Error> {
    match value.inner {
      Either::A(s) => rolldown_sourcemap::SourceMap::from_json_string(&s)
        .map_err(|e| anyhow::format_err!("Convert string sourcemap error: {:?}", e)),
      Either::B(v) => v.try_into(),
    }
  }
}

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
  type Error = anyhow::Error;

  fn try_from(value: BindingJsonSourcemap) -> Result<Self, Self::Error> {
    let map = rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
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
    .map_err(|e| anyhow::format_err!("Convert json sourcemap error: {:?}", e))?;
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
