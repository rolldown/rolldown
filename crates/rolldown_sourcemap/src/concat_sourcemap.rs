use parcel_sourcemap::SourceMap as ParcelSourcemap;

use crate::SourceMap;

pub fn concat_sourcemaps(
  content_and_sourcemaps: &[(&str, Option<&SourceMap>)],
) -> Result<SourceMap, String> {
  let mut map = ParcelSourcemap::new("");
  let mut line_offset = 0;

  for (content, sourcemap) in content_and_sourcemaps {
    if let Some(sourcemap) = sourcemap {
      map
        .add_sourcemap(
          &mut sourcemap.get_inner().cloned().ok_or("concat sourcemap not inner sourcemap")?,
          line_offset.into(),
        )
        .map_err(|e| e.to_string())?;
    }
    line_offset += u32::try_from(content.lines().count()).map_err(|e| e.to_string())?;
  }

  Ok(map.into())
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
      ("\nconsole.log()", None),
      ("function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n", Some(&map)),
    ];

    let result =
      super::concat_sourcemaps(&content_and_sourcemaps).expect("should not fail").to_json();

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
    "mappings": ";;AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC"
  }"#;
    assert_eq!(
      result.as_str().parse::<serde_json::Value>().unwrap(),
      expected.parse::<serde_json::Value>().unwrap()
    );
  }
}
