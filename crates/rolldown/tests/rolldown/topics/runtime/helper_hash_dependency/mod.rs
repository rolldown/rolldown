use std::{borrow::Cow, path::PathBuf};

use rolldown::{Bundler, BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::{Output, RUNTIME_MODULE_KEY};
use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
struct RuntimeVersion(u8);

impl Plugin for RuntimeVersion {
  fn name(&self) -> Cow<'static, str> {
    "runtime-version".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    Ok((args.id == RUNTIME_MODULE_KEY).then(|| rolldown_plugin::HookTransformOutput {
      code: Some(format!("{}\nglobalThis.runtimeVersion = {};", args.code, self.0)),
      ..Default::default()
    }))
  }
}

async fn lazy_filename(version: u8) -> String {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      cwd: Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"),
        "/tests/rolldown/topics/hmr/runtime_helpers_survive_entry_failure"))),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      entry_filenames: Some("[name]-[hash].js".to_string().into()),
      chunk_filenames: Some("[name]-[hash].js".to_string().into()),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions {
          implement: Some("globalThis.__rolldown_runtime__ = Object.freeze({ registerGraph() {}, registerModule() {}, createModuleHotContext() { return {}; } });".into()),
          ..Default::default()
        }),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(RuntimeVersion(version))],
  ).unwrap();
  let output = bundler.generate().await.unwrap();
  assert_eq!(output.assets.iter().filter(|asset| matches!(asset, Output::Chunk(_))).count(), 3);
  output
    .assets
    .into_iter()
    .find_map(|asset| match asset {
      Output::Chunk(chunk) if chunk.filename.starts_with("lazy-") => {
        Some(chunk.filename.to_string())
      }
      _ => None,
    })
    .expect("lazy chunk")
}

#[tokio::test(flavor = "multi_thread")]
async fn helper_changes_invalidate_lazy_chunk_hash() {
  assert_ne!(lazy_filename(1).await, lazy_filename(2).await);
}
