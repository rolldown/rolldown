use std::borrow::Cow;

use rolldown::{BundlerOptions, OutputFormat, PreserveEntrySignatures};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{__inner::SharedPluginable, HookUsage, Plugin, PluginContext};
use rolldown_testing::integration_test::NamedBundlerOptions;

#[derive(Debug)]
struct EmitChunks(&'static [&'static str]);

impl Plugin for EmitChunks {
  fn name(&self) -> Cow<'static, str> {
    "emit-chunk-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    for &id in self.0 {
      ctx.emit_chunk(EmittedChunk {
        id: id.to_string(),
        preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
        ..Default::default()
      })?;
    }
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

pub(super) fn emit_chunks(ids: &'static [&'static str]) -> SharedPluginable {
  EmitChunks::new_shared(EmitChunks(ids))
}

pub(super) fn es_and_cjs(options: BundlerOptions) -> Vec<NamedBundlerOptions> {
  vec![
    NamedBundlerOptions {
      description: None,
      options: options.clone(),
      snapshot: None,
      config_name: None,
      expect_execution_failure: None,
    },
    NamedBundlerOptions {
      description: Some("cjs".to_string()),
      options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options },
      snapshot: None,
      config_name: Some("cjs".to_string()),
      expect_execution_failure: None,
    },
  ]
}
