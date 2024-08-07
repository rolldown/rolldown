use derivative::Derivative;
use napi::bindgen_prelude::Either3;
use rolldown_sourcemap::{MissingSourceMap, SourceMapOrMissing};
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Derivative)]
pub struct BindingSourcemap {
  #[serde(skip_deserializing, default = "default_sourcemap")]
  pub inner: Either3<String, BindingJsonSourcemap, BindingJsonMissingSourcemap>,
}

fn default_sourcemap() -> Either3<String, BindingJsonSourcemap, BindingJsonMissingSourcemap> {
  Either3::A(String::default())
}

impl TryFrom<BindingSourcemap> for rolldown_sourcemap::SourceMapOrMissing {
  type Error = anyhow::Error;

  fn try_from(value: BindingSourcemap) -> Result<Self, Self::Error> {
    match value.inner {
      Either3::A(s) => rolldown_sourcemap::SourceMap::from_json_string(&s)
        .map(SourceMapOrMissing::ExistingSourceMap)
        .map_err(|e| anyhow::format_err!("Convert string sourcemap error: {:?}", e)),
      Either3::B(v) => v.try_into(),
      Either3::C(v) => v.try_into(),
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

impl TryFrom<BindingJsonSourcemap> for rolldown_sourcemap::SourceMapOrMissing {
  type Error = anyhow::Error;

  fn try_from(value: BindingJsonSourcemap) -> Result<Self, Self::Error> {
    rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
      file: value.file,
      mappings: value.mappings,
      source_root: value.source_root,
      sources: value.sources,
      sources_content: value.sources_content,
      names: value.names,
    })
    .map(SourceMapOrMissing::ExistingSourceMap)
    .map_err(|e| anyhow::format_err!("Convert json sourcemap error: {:?}", e))
  }
}

#[derive(Deserialize, Debug, Default, Derivative)]
#[napi_derive::napi(object)]
pub struct BindingJsonMissingSourcemap {
  pub missing: bool,
  pub plugin_name: Option<String>,
}

impl TryFrom<BindingJsonMissingSourcemap> for rolldown_sourcemap::SourceMapOrMissing {
  type Error = anyhow::Error;

  fn try_from(value: BindingJsonMissingSourcemap) -> Result<Self, Self::Error> {
    Ok(SourceMapOrMissing::MissingSourceMap(MissingSourceMap {
      missing: value.missing,
      plugin_name: value.plugin_name.map(Into::into),
    }))
  }
}
