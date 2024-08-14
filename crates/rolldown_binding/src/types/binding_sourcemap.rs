use derivative::Derivative;
use napi::Either;
use rolldown_sourcemap::{JSONSourceMap, MissingSourceMap, SourceMap, SourceMapOrMissing};
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

impl TryFrom<BindingSourcemap> for SourceMapOrMissing {
  type Error = anyhow::Error;

  fn try_from(value: BindingSourcemap) -> Result<Self, Self::Error> {
    match value.inner {
      Either::A(s) => SourceMap::from_json_string(&s)
        .map(SourceMapOrMissing::SourceMap)
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
  pub missing: Option<bool>,
  pub plugin_name: Option<String>,
}

impl TryFrom<BindingJsonSourcemap> for SourceMapOrMissing {
  type Error = anyhow::Error;

  fn try_from(value: BindingJsonSourcemap) -> Result<Self, Self::Error> {
    if let Some(missing) = value.missing {
      if missing {
        return Ok(SourceMapOrMissing::Missing(MissingSourceMap {
          plugin_name: value.plugin_name.map(Into::into),
        }));
      }
    }
    SourceMap::from_json(JSONSourceMap {
      file: value.file,
      mappings: value.mappings,
      source_root: value.source_root,
      sources: value.sources,
      sources_content: value.sources_content,
      names: value.names,
    })
    .map(SourceMapOrMissing::SourceMap)
    .map_err(|e| anyhow::format_err!("Convert json sourcemap error: {:?}", e))
  }
}
