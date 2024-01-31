use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derive(Debug)]
pub struct SourceMap {
  // pub file: Option<String>,
  pub mappings: String,
  pub names: Vec<String>,
  pub source_root: Option<String>,
  pub sources: Vec<String>,
  pub sources_content: Vec<String>,
  // pub version: u32,
  // #[serde(rename = "x_google_ignoreList")]
  // pub x_google_ignore_list: Option<Vec<u32>>,
}

impl From<SourceMap> for rolldown_sourcemap::SourceMap {
  fn from(value: SourceMap) -> Self {
    Self::new(value.mappings, value.names, value.source_root, value.sources, value.sources_content)
  }
}
