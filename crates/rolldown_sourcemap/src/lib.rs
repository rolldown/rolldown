pub use parcel_sourcemap::SourceMap;

pub fn collapse_sourcemaps(mut sourcemaps: Vec<SourceMap>) -> Result<SourceMap, String> {
  sourcemaps.reverse();

  let Some(mut result) = sourcemaps.pop() else { return Ok(SourceMap::new("")) };

  for mut sourcemap in sourcemaps.into_iter().rev() {
    if let Err(e) = sourcemap.extends(&mut result) {
      return Err(format!("{e}"));
    };
    result = sourcemap;
  }

  Ok(result)
}

#[cfg(test)]
mod tests {
  use crate::SourceMap;
  use serde_json;
  #[test]
  fn it_works() {
    let sourcemaps = vec![
      SourceMap::from_json(
        "/",
        r#"{
      "version": 3,
      "file": "index.js",
      "sourceRoot": "",
      "sources": [
        "index.ts"
      ],
      "names": [],
      "mappings": "AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC",
      "sourcesContent": [
        "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
      ]
  }"#,
      ).unwrap(),
      SourceMap::from_json(
        "/",
        r#"{
          "version": 3,
          "file": "minify.js",
          "sourceRoot": "",
          "sources": [
            "index.ts"
          ],
          "names": [
            "sayHello",
            "name",
            "console",
            "log",
            "concat"
          ],
          "mappings": "AAAA,SAASA,SAASC,CAAI,EAClBC,QAAQC,GAAG,CAAC,UAAUC,MAAM,CAACH,GACjC",
          "sourcesContent": [
            "function sayHello(name) {\n    console.log(\"Hello, \".concat(name));\n}\n"
          ]
      }"#,
      ).unwrap(),
    ];

    let result =
      super::collapse_sourcemaps(sourcemaps).expect("should not fail").to_json(None).unwrap();

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
