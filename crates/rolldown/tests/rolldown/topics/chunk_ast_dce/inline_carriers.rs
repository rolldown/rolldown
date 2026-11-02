use std::{borrow::Cow, path::Path, process::Command, sync::Arc};

use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin,
  SharedTransformPluginContext,
};
use string_wizard::{MagicString, SourceMapOptions};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  CodeSplittingMode, InlineCommonChunksOptions, ManualCodeSplittingOptions, Output,
  PreserveEntrySignatures, RawCompressOptions, RawMangleOptions, RawMinifyOptions,
  RawMinifyOptionsDetailed,
};

const BASE: &str = "globalThis.events.push('inline-base-anchor');\nexport let count = 0;\nexport function bump() { count++; }\nexport function bare() { return this; }\nexport const identity = {};\nexport class Secret { #value = 7; read() { return this.#value; } }\n";
const RECORD: &str = "import * as base from './base.js';\nglobalThis.events.push('inline-record-anchor');\nimport lib from './lib.cjs';\nexport { answer } from './lib.cjs';\nconst protoValue = { safe: true };\nexport { protoValue as __proto__ };\nconst value = 11;\nexport function read() { return [base.count, base.bare(), new base.Secret().read(), value, lib.answer]; }\nexport function update() { base.bump(); }\nexport function id() { return base.identity; }\nexport function load() { return import('./lazy.js'); }\n";
const LAZY: &str = "globalThis.events.push('inline-lazy-anchor');\nexport const result = 29;\n";

#[derive(Debug)]
struct ShiftInlineProducers;

impl Plugin for ShiftInlineProducers {
  fn name(&self) -> Cow<'static, str> {
    "shift-inline-producers".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with("base.js") && !args.id.ends_with("record.js") {
      return Ok(None);
    }
    let mut source = MagicString::new(args.code.as_str());
    source.prepend("// transformed inline donor\n// extra source line\n");
    Ok(Some(HookTransformOutput {
      code: Some(source.to_string()),
      map: source
        .source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: Arc::from(args.id),
        })
        .into(),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn inline_carriers_preserve_factories_live_reads_and_maps() {
  let pipeline = "chunk-ast";
  for mode in ["none", "dce", "compress", "mangle", "full"] {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/inline-carriers")
      .join(pipeline)
      .join(mode);
    let input_directory = directory.join("input");
    let output_directory = directory.join("dist");
    std::fs::create_dir_all(&input_directory).unwrap();
    std::fs::create_dir_all(&output_directory).unwrap();
    for (name, source) in [("base.js", BASE), ("record.js", RECORD), ("lazy.js", LAZY)] {
      std::fs::write(input_directory.join(name), source).unwrap();
    }
    std::fs::write(input_directory.join("lib.cjs"), "module.exports = { answer: 31 };\n").unwrap();
    for name in ["a", "b"] {
      std::fs::write(input_directory.join(format!("{name}.js")), format!(
        "import {{ read, update, id, load, answer, __proto__ as protoValue }} from './record.js';\nconst value = '{name}';\nglobalThis.{name} = {{ read, update, id, load, answer, protoValue, carrier: value }};\nglobalThis.events.push('inline-{name}-anchor');\n"
      )).unwrap();
    }
    std::fs::write(input_directory.join("c.js"), "import { count, bump, identity } from './base.js';\nglobalThis.c = { read: () => count, bump, identity };\nglobalThis.events.push('inline-c-anchor');\n").unwrap();
    let minify = match mode {
      "none" => RawMinifyOptions::Bool(false),
      "dce" => RawMinifyOptions::DeadCodeEliminationOnly,
      "full" => RawMinifyOptions::Bool(true),
      _ => RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        compress: (mode == "compress").then(RawCompressOptions::default),
        mangle: (mode == "mangle").then(RawMangleOptions::default),
        mangle_properties: None,
        remove_whitespace: mode == "mangle",
        ascii_only: false,
      }),
    };
    let output = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(input_directory),
        input: Some(
          ["a", "b", "c"]
            .into_iter()
            .map(|name| InputItem {
              name: Some(format!("{name}/entry")),
              import: format!("./{name}.js"),
            })
            .collect(),
        ),
        entry_filenames: Some("[name].mjs".to_string().into()),
        chunk_filenames: Some("chunks/[name]-[hash].mjs".to_string().into()),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          experimental_inline_common_chunks: Some(InlineCommonChunksOptions {
            max_size: Some(f64::INFINITY),
            exclude: None,
          }),
          ..Default::default()
        })),
        minify: Some(minify),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![Plugin::new_shared(ShiftInlineProducers)],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    let mut base_copies = 0;
    let mut record_copies = 0;
    for asset in &output.assets {
      let Output::Chunk(chunk) = asset else { continue };
      let filename = output_directory.join(chunk.filename.as_str());
      std::fs::create_dir_all(filename.parent().unwrap()).unwrap();
      std::fs::write(&filename, &chunk.code).unwrap();
      let Some(map) = &chunk.map else {
        assert!(
          !chunk.code.contains("inline-base-anchor")
            && !chunk.code.contains("inline-record-anchor"),
          "real inline donors need source mappings"
        );
        continue;
      };
      let map_json = map.to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&map_json).unwrap();
      super::map_assertions::assert_valid_map(&chunk.code, &map);
      for (marker, source, content, count) in [
        ("inline-base-anchor", "base.js", BASE, &mut base_copies),
        ("inline-record-anchor", "record.js", RECORD, &mut record_copies),
      ] {
        if chunk.code.contains(marker) {
          *count += 1;
          super::map_assertions::assert_mapping(
            &chunk.code,
            &map,
            marker,
            source,
            content,
            &format!("'{marker}'"),
          );
        }
      }
      if mode == "dce" && chunk.code.contains("__share(") {
        super::map_assertions::assert_unmapped(&chunk.code, &map, "__share(");
      }
      std::fs::write(filename.with_extension("mjs.map"), map_json).unwrap();
    }
    assert!(base_copies >= 2, "base must be copied into independent carriers");
    assert!(record_copies >= 2, "record must be copied into independent carriers");
    for order in [["a", "b", "c"], ["c", "b", "a"]] {
      let runner = output_directory.join("execute.mjs");
      std::fs::write(&runner, format!(
        "import assert from 'node:assert/strict';\nglobalThis.events = [];\nfor (const name of {}) await import('./' + name + '/entry.mjs');\nassert.deepEqual(a.read(), [0, undefined, 7, 11, 31]);\nassert.deepEqual(b.read(), [0, undefined, 7, 11, 31]);\na.update();\nassert.equal(c.read(), 1);\nc.bump();\nassert.equal(b.read()[0], 2);\nassert.equal(a.id(), b.id());\nassert.equal(a.id(), c.identity);\nassert.equal(a.answer, 31);\nassert.equal(b.answer, 31);\nassert.deepEqual(a.protoValue, {{ safe: true }});\nassert.equal(a.protoValue, b.protoValue);\nassert.equal(a.carrier, 'a');\nassert.equal(b.carrier, 'b');\nassert.equal((await a.load()).result, 29);\nassert.equal((await b.load()).result, 29);\nfor (const marker of ['inline-base-anchor', 'inline-record-anchor', 'inline-lazy-anchor']) assert.equal(events.filter(event => event === marker).length, 1, marker);\nassert.ok(events.indexOf('inline-base-anchor') < events.indexOf('inline-record-anchor'));\n",
        serde_json::to_string(&order).unwrap()
      )).unwrap();
      let executed = Command::new("node").arg(&runner).output().unwrap();
      assert!(
        executed.status.success(),
        "{pipeline}/{mode}/{order:?}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
    }
  }
}
