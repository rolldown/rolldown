use parcel_sourcemap::SourceMap as ParcelSourcemap;
use rolldown_error::BuildError;

use crate::SourceMap;

pub fn concat_sourcemaps(
  content_and_sourcemaps: &[(String, Option<SourceMap>)],
) -> Result<(String, SourceMap), BuildError> {
  let mut s = String::new();
  let mut map = ParcelSourcemap::new("");
  let mut line_offset = 0;

  for (index, (content, sourcemap)) in content_and_sourcemaps.iter().enumerate() {
    s.push_str(content);
    if index != content_and_sourcemaps.len() - 1 {
      s.push('\n');
    }

    if let Some(sourcemap) = sourcemap {
      map
        .add_sourcemap(
          &mut sourcemap.get_inner().cloned().ok_or(BuildError::sourcemap_error(
            "concat sourcemap not inner sourcemap".to_string(),
          ))?,
          line_offset.into(),
        )
        .map_err(|e| BuildError::sourcemap_error(e.to_string()))?;
    }
    line_offset += u32::try_from(content.lines().count() + 1)
      .map_err(|e| BuildError::sourcemap_error(e.to_string()))?;
  }

  Ok((s, map.into()))
}

#[cfg(test)]
mod tests {
  use parcel_sourcemap::SourceMap;
  use serde_json;
  #[test]
  fn concat_sourcemaps_works() {
    let map = SourceMap::from_json(
        "",
        r#"{
          "version":3,
          "sourceRoot":"",
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

    let (content, mut map) =
      super::concat_sourcemaps(&content_and_sourcemaps).expect("should not fail");

    assert_eq!(
      content,
      "\nconsole.log()\nfunction sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    );
    let expected = r#"{
      "version": 3,
      "sources": [
      "index.ts"
    ],
    "sourceRoot": null,
    "sourcesContent": [
        "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    ],
    "names": [],
    "mappings": ";;;AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC"
  }"#;
    assert_eq!(
      map.to_json().as_str().parse::<serde_json::Value>().unwrap(),
      expected.parse::<serde_json::Value>().unwrap()
    );
  }
}
