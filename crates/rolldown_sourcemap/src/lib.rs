use parcel_sourcemap::SourceMap as ParcelSourcemap;

#[derive(Debug, Default, Clone)]
pub struct SourceMap {
  pub mappings: String,
  pub names: Vec<String>,
  pub source_root: Option<String>,
  pub sources: Vec<String>,
  pub sources_content: Vec<String>,
  inner: Option<ParcelSourcemap>,
}

impl SourceMap {
  pub fn new(
    mappings: String,
    names: Vec<String>,
    source_root: Option<String>,
    sources: Vec<String>,
    sources_content: Vec<String>,
  ) -> Self {
    Self { mappings, names, source_root, sources, sources_content, inner: None }
  }

  pub fn to_json(&mut self) -> String {
    self.inner.as_mut().expect("should have inner").to_json(None).expect("should success")
  }

  pub fn to_data_url(&mut self) -> String {
    self.inner.as_mut().expect("should have inner").to_data_url(None).expect("should success")
  }
}

impl From<ParcelSourcemap> for SourceMap {
  fn from(value: ParcelSourcemap) -> Self {
    Self { inner: Some(value), ..Default::default() }
  }
}

pub fn collapse_sourcemaps(sourcemap_chain: Vec<SourceMap>) -> Result<Option<SourceMap>, String> {
  let mut parcel_sourcemap_chain = sourcemap_chain
    .into_iter()
    .map(|sourcemap| {
      let mut map = ParcelSourcemap::new(sourcemap.source_root.as_deref().unwrap_or(""));
      if let Err(e) = map.add_vlq_map(
        sourcemap.mappings.as_bytes(),
        sourcemap.sources,
        sourcemap.sources_content,
        sourcemap.names,
        0,
        0,
      ) {
        return Err(format!("{e}"));
      }
      Ok(map)
    })
    .rev()
    .collect::<Result<Vec<_>, String>>()?;

  let Some(mut result) = parcel_sourcemap_chain.pop() else { return Ok(None) };

  for mut sourcemap in parcel_sourcemap_chain.into_iter().rev() {
    if let Err(e) = sourcemap.extends(&mut result) {
      return Err(format!("{e}"));
    };
    result = sourcemap;
  }

  Ok(Some(result.into()))
}

#[cfg(test)]
mod tests {
  use crate::SourceMap;
  use serde_json;
  #[test]
  fn it_works() {
    let sourcemaps = vec![
      SourceMap::new(
        "AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC"
          .to_string(),
        vec![],
        None,
        vec!["index.ts".to_string()],
        vec!["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n".to_string()],
      ),
      SourceMap::new(
        "AAAA,SAASA,SAASC,CAAI,EAClBC,QAAQC,GAAG,CAAC,UAAUC,MAAM,CAACH,GACjC".to_string(),
        vec![
          "sayHello".to_string(),
          "name".to_string(),
          "console".to_string(),
          "log".to_string(),
          "concat".to_string(),
        ],
        None,
        vec!["index.ts".to_string()],
        vec![
          "function sayHello(name) {\n    console.log(\"Hello, \".concat(name));\n}\n".to_string()
        ],
      ),
    ];

    let result =
      super::collapse_sourcemaps(sourcemaps).expect("should not fail").unwrap().to_json();

    let expected = r#"{
      "version": 3,
      "sources": [
      "index.ts"
    ],
    "sourceRoot": null,
    "sourcesContent": [
      "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    ],
    "names": [
      "sayHello",
      "name",
      "console",
      "log",
      "concat"
    ],
    "mappings": "AAAA,SAAS,SAAS,CAAY,EAC5B,QAAQ,GAAG,CAAC,UAAA,MAAA,CAAU,GACxB"
  }"#;
    assert_eq!(
      result.as_str().parse::<serde_json::Value>().unwrap(),
      expected.parse::<serde_json::Value>().unwrap()
    );
  }
}
