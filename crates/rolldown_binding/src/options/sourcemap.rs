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

impl From<SourceMap> for napi::Result<rolldown_sourcemap::SourceMap> {
  fn from(value: SourceMap) -> Self {
    let mut map = rolldown_sourcemap::SourceMap::new(value.source_root.as_deref().unwrap_or(""));
    if let Err(e) = map.add_vlq_map(
      value.mappings.as_bytes(),
      value.sources,
      value.sources_content,
      value.names,
      0,
      0,
    ) {
      return Err(napi::Error::from_reason(format!("{e}")));
    }
    Ok(map)
  }
}
