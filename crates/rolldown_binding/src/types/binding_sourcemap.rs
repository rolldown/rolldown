use derivative::Derivative;
use napi::Either;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Derivative)]
pub struct BindingSourcemap {
  #[serde(skip_deserializing, default = "default_sourcemap")]
  pub inner: Either<String, BindingJsonSourcemap>,
}

fn default_sourcemap() -> Either<String, BindingJsonSourcemap> {
  Either::A(String::default())
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

#[derive(Deserialize, Debug, Default, Derivative)]
#[napi_derive::napi(object)]
pub struct BindingJsonSourcemap {
  pub file: Option<String>,
  pub mappings: Option<String>,
  pub source_root: Option<String>,
  pub sources: Option<Vec<Option<String>>>,
  pub sources_content: Option<Vec<Option<String>>>,
  pub names: Option<Vec<String>>,
}

impl TryFrom<BindingJsonSourcemap> for rolldown_sourcemap::SourceMap {
  type Error = anyhow::Error;

  fn try_from(value: BindingJsonSourcemap) -> Result<Self, Self::Error> {
    rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
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
      debug_id: None,
    })
    .map_err(|e| anyhow::format_err!("Convert json sourcemap error: {:?}", e))
  }
}
