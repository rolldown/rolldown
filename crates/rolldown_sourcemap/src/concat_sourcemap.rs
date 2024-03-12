// cSpell:disable
use rolldown_error::BuildError;
use sourcemap::{SourceMap, SourceMapBuilder};

pub fn concat_sourcemaps(
  content_and_sourcemaps: &[(String, Option<SourceMap>)],
) -> Result<(String, SourceMap), BuildError> {
  let mut s = String::new();
  let mut sourcemap_builder = SourceMapBuilder::new(None);
  let mut line_offset = 0;

  for (index, (content, sourcemap)) in content_and_sourcemaps.iter().enumerate() {
    s.push_str(content);
    if index < content_and_sourcemaps.len() - 1 {
      s.push('\n');
    }

    if let Some(sourcemap) = sourcemap {
      for (id, source) in sourcemap.sources().enumerate() {
        let source_id = sourcemap_builder.add_source(source);
        sourcemap_builder.set_source_contents(source_id, sourcemap.get_source_contents(id as u32));
      }
      for token in sourcemap.tokens() {
        sourcemap_builder.add(
          token.get_dst_line() + line_offset,
          token.get_dst_col(),
          token.get_src_line(),
          token.get_src_col(),
          token.get_source(),
          token.get_name(),
        );
      }
    }
    line_offset += u32::try_from(content.lines().count() + 1)
      .map_err(|e| BuildError::sourcemap_error(e.to_string()))?;
  }

  Ok((s, sourcemap_builder.into_sourcemap()))
}

#[cfg(test)]
mod tests {
  pub use sourcemap::SourceMap;
  #[test]
  fn concat_sourcemaps_works() {
    let map = SourceMap::from_slice(
        r#"{
          "version":3,
          "sourceRoot":"",
          "mappings":"AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC",
          "sources":["index.ts"],
          "sourcesContent":["function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"],
          "names":[]
        }"#.as_bytes()
    )
    .unwrap();
    let content_and_sourcemaps = vec![
      ("\nconsole.log()".to_string(), None),
      (
        "function sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n".to_string(),
        Some(map),
      ),
    ];

    let (content, map) = {
      let (content, map) =
        super::concat_sourcemaps(&content_and_sourcemaps).expect("should not fail");
      let mut buf = vec![];
      map.to_writer(&mut buf).unwrap();
      (content, unsafe { String::from_utf8_unchecked(buf) })
    };

    assert_eq!(
      content,
      "\nconsole.log()\nfunction sayHello(name: string) {\n  console.log(`Hello, ${name}`);\n}\n"
    );
    let expected = "{\"version\":3,\"sources\":[\"index.ts\"],\"sourcesContent\":[\"function sayHello(name: string) {\\n  console.log(`Hello, ${name}`);\\n}\\n\"],\"names\":[],\"mappings\":\";;;AAAA,SAAS,QAAQ,CAAC,IAAY;IAC5B,OAAO,CAAC,GAAG,CAAC,iBAAU,IAAI,CAAE,CAAC,CAAC;AAChC,CAAC\"}";
    assert_eq!(map, expected);
  }
}
