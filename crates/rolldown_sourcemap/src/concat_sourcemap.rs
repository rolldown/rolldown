// cSpell:disable
use parcel_sourcemap::SourceMap as ParcelSourcemap;
use rolldown_error::BuildError;

use crate::SourceMap;

pub fn concat_sourcemaps(
  content_and_sourcemaps: &[(String, Option<SourceMap>)],
) -> Result<(String, SourceMap), BuildError> {
  let line_offsets: Vec<usize> =
    content_and_sourcemaps.iter().map(|(content, _)| content.lines().count() + 1).collect();

  let sourcemap_content = content_and_sourcemaps
    .iter()
    .map(|(content, _)| content.as_str())
    .collect::<Vec<&str>>()
    .join("\n");

  let mut map = ParcelSourcemap::new("");
  let mut line_offset = 0;

  for (index, (_, sourcemap_option)) in content_and_sourcemaps.iter().enumerate() {
    if let Some(sourcemap) = sourcemap_option {
      map
        .add_sourcemap(
          &mut sourcemap.get_inner().cloned().ok_or(BuildError::sourcemap_error(
            "concat sourcemap not inner sourcemap".to_string(),
          ))?,
          line_offset as i64,
        )
        .map_err(|e| BuildError::sourcemap_error(e.to_string()))?;
    }

    line_offset += line_offsets[index];
  }

  Ok((sourcemap_content, map.into()))
}

#[cfg(test)]
mod tests {
  use parcel_sourcemap::SourceMap;
  use serde_json::{self, Value};
  #[test]
  fn concat_sourcemaps_works() {
    let map = SourceMap::from_json(
          "",
          r#"{
              "version":3,
              "sourceRoot":null,
              "mappings":"AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC",
              "sources":["index.ts"],
              "sourcesContent":["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"],
              "names":[]
          }"#,
      )
      .unwrap().into();

    let content_and_sourcemaps = vec![
      ("\nconsole.log()".to_string(), None),
      (
        "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n".to_string(),
        Some(map),
      ),
    ];

    let (content, mut map_result) =
      super::concat_sourcemaps(&content_and_sourcemaps).expect("should not fail");

    assert_eq!(
      content,
      "\nconsole.log()\nfunction sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    );

    let expected_json = serde_json::from_str::<Value>(r#"{
          "version": 3,
          "sources": ["index.ts"],
          "sourceRoot": null,
          "sourcesContent": ["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"],
          "names": [],
          "mappings": ";;;AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC"
      }"#).unwrap();

    match map_result.to_json() {
      Some(Ok(json_string)) => {
        let actual_json = serde_json::from_str::<Value>(&json_string).unwrap();
        assert_eq!(actual_json, expected_json);
      }
      Some(Err(e)) => panic!("Error generating JSON: {:?}", e),
      None => panic!("JSON generation resulted in None"),
    }
  }
}
