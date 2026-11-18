use std::{fs, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat, Platform, plugin::Pluginable};
use rolldown_common::{Output, RawMinifyOptions};
use rolldown_plugin_vite_json::{ViteJsonPlugin, ViteJsonPluginStringify};

#[tokio::test]
async fn preserve_json_utf16_strings_and_properties() {
  let directory = tempfile::tempdir().unwrap();
  let cwd = directory.path().canonicalize().unwrap();
  let configurations =
    std::iter::once(None).chain([false, true].into_iter().flat_map(|named_exports| {
      [false, true].into_iter().flat_map(move |minify| {
        [ViteJsonPluginStringify::False, ViteJsonPluginStringify::True]
          .into_iter()
          .map(move |stringify| Some((named_exports, minify, stringify)))
      })
    }));
  for plugin_options in configurations {
    for format in [OutputFormat::Esm, OutputFormat::Cjs] {
      for (json, check_named_exports) in [
        (
          r#"{"high":"\ud800","low":"\udfff","pair":"\ud83d\ude00","highHighLow":"\ud800\ud800\udc00","escaped":"\\ud800","object":{"nested":"\ud800","__proto__":{"polluted":true}},"__proto__":0,"last":false,"\u005f_proto__":{"marker":"\udfff"},"\ud800":1,"\uD800":2,"JSON":null,"large":9007199254740995,"huge":1e400,"negativeZero":-0}"#,
          true,
        ),
        (r#"["\ud800","\udfff","\ud83d\ude00",{"\ud800":"\udfff"}]"#, false),
        (r#""\ud800""#, false),
        (r#"{"value":{"nested":["\ud800"]},"value":1}"#, false),
      ] {
        let named_exports = plugin_options.is_none_or(|options| options.0);
        let named_assertions = if check_named_exports {
          if named_exports {
            "assert.equal(ns.high, data.high); assert.equal(ns.low, data.low); assert.equal(ns.object, data.object);"
          } else {
            "assert.deepEqual(Object.keys(ns), ['default']);"
          }
        } else {
          ""
        };
        fs::write(cwd.join("data.json"), json).unwrap();
        fs::write(
          cwd.join("main.js"),
          format!(
            "import assert from 'node:assert/strict';\nimport data, * as ns from './data.json';\nassert.deepStrictEqual(data, JSON.parse({}));\n{named_assertions}",
            serde_json::to_string(json).unwrap()
          ),
        )
        .unwrap();

        let plugins = plugin_options
          .map(|(named_exports, minify, stringify)| {
            vec![Pluginable::new_shared(ViteJsonPlugin { named_exports, minify, stringify })]
          })
          .unwrap_or_default();
        let mut bundler = Bundler::with_plugins(
          BundlerOptions {
            input: Some(vec![InputItem { import: "./main.js".to_string(), ..Default::default() }]),
            cwd: Some(cwd.clone()),
            platform: Some(Platform::Node),
            format: Some(format),
            minify: Some(RawMinifyOptions::Bool(plugin_options.is_some_and(|options| options.1))),
            ..Default::default()
          },
          plugins,
        )
        .unwrap();
        let output = bundler.generate().await.unwrap_or_else(|error| {
          panic!("plugin={plugin_options:?}, format={format:?}, json={json}: {error:?}")
        });
        let [Output::Chunk(chunk)] = output.assets.as_slice() else {
          panic!("expected one output chunk");
        };
        let result = Command::new("node")
          .args([
            if matches!(format, OutputFormat::Esm) {
              "--input-type=module"
            } else {
              "--input-type=commonjs"
            },
            "--eval",
            chunk.code.as_str(),
          ])
          .output()
          .unwrap();
        assert!(
          result.status.success(),
          "plugin={plugin_options:?}, format={format:?}, json={json}\n{}\n{}",
          String::from_utf8_lossy(&result.stderr),
          chunk.code
        );
        bundler.close().await.unwrap();
      }
    }
  }
}
