use std::{fs, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, Platform, plugin::Pluginable};
use rolldown_common::{Output, RawMinifyOptions};
use rolldown_plugin_vite_json::{ViteJsonPlugin, ViteJsonPluginStringify};

#[tokio::test]
async fn preserve_proto_data_properties() {
  let directory = tempfile::tempdir().unwrap();
  let cwd = directory.path().canonicalize().unwrap();
  for named_exports in [false, true] {
    for minify in [false, true] {
      for json in [
        r#"{"__proto__":0,"nested":{"__proto__":{"marker":42}},"nil":{"__proto__":null},"array":[{"__proto__":false}],"JSON":null}"#,
        r#"[{"__proto__":"value"},{"nested":{"__proto__":42}}]"#,
        r#"{"prefix\"__proto__":{"__proto__":42},"text":"{\"__proto__\":null}"}"#,
      ] {
        fs::write(cwd.join("data.json"), json).unwrap();
        fs::write(
          cwd.join("main.js"),
          format!(
            "import assert from 'node:assert/strict';\nimport data from './data.json';\nassert.deepStrictEqual(data, JSON.parse({}));",
            serde_json::to_string(json).unwrap()
          ),
        )
        .unwrap();

        let mut bundler = Bundler::with_plugins(
          BundlerOptions {
            input: Some(vec![InputItem { import: "./main.js".to_string(), ..Default::default() }]),
            cwd: Some(cwd.clone()),
            platform: Some(Platform::Node),
            minify: Some(RawMinifyOptions::Bool(minify)),
            ..Default::default()
          },
          vec![Pluginable::new_shared(ViteJsonPlugin {
            named_exports,
            stringify: ViteJsonPluginStringify::False,
            ..Default::default()
          })],
        )
        .unwrap();
        let output = bundler.generate().await.unwrap();
        let [Output::Chunk(chunk)] = output.assets.as_slice() else {
          panic!("expected one output chunk");
        };
        let result = Command::new("node")
          .args(["--input-type=module", "--eval", chunk.code.as_str()])
          .output()
          .unwrap();
        assert!(
          result.status.success(),
          "named_exports={named_exports}, minify={minify}, json={json}\n{}",
          String::from_utf8_lossy(&result.stderr)
        );
        bundler.close().await.unwrap();
      }
    }
  }
}
