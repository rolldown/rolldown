use std::borrow::Cow;

use rolldown::{AttachDebugInfo, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext,
};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};

/// Loads a `<path>?<query>` id from `<path>`, like Vite's load fallback.
#[derive(Debug)]
struct QueryLoadPlugin;

impl Plugin for QueryLoadPlugin {
  fn name(&self) -> Cow<'static, str> {
    "query-load".into()
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let Some((path, _query)) = args.id.split_once('?') else {
      return Ok(None);
    };
    Ok(Some(HookLoadOutput { code: std::fs::read_to_string(path)?.into(), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}

// A line terminator in a module id must not end the debug-info comment that holds the id.
#[tokio::test(flavor = "multi_thread")]
async fn line_terminator_in_module_id() {
  let options = BundlerOptions {
    input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
    experimental: Some(ExperimentalOptions {
      attach_debug_info: Some(AttachDebugInfo::Full),
      ..Default::default()
    }),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta::default())
    .run_multiple(
      vec![
        NamedBundlerOptions {
          description: None,
          options: options.clone(),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        // Each module gets its own chunk, and its `stable_id` goes in the `//!` header.
        NamedBundlerOptions {
          description: Some("preserveModules".to_string()),
          options: BundlerOptions { preserve_modules: Some(true), ..options.clone() },
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("minify".to_string()),
          options: BundlerOptions { minify: Some(true.into()), ..options.clone() },
          snapshot: Some(false),
          config_name: None,
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("preserveModules + minify".to_string()),
          options: BundlerOptions {
            preserve_modules: Some(true),
            minify: Some(true.into()),
            ..options
          },
          snapshot: Some(false),
          config_name: None,
          expect_execution_failure: None,
        },
      ],
      vec![Plugin::new_shared(QueryLoadPlugin)],
    )
    .await;
}
