use std::sync::Arc;

use rolldown::BundlerOptions;
use rolldown_common::{ExperimentalOptions, HmrOptions};
use rolldown_testing::{abs_file_dir, manual_integration_test, test_config::TestMeta};

#[tokio::test(flavor = "multi_thread")]
async fn continuous_generate_hmr_patch() {
  // https://github.com/vitejs/rolldown-vite/blob/942cb2b51b59fd6aefe886ec78eb34fff56ead34/playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts#L103-L122
  let dir = abs_file_dir!();
  let hmr_temp = dir.join("hmr-temp");
  let hmr_js = hmr_temp.join("hmr.js");

  let (testing_plugin, _) = rolldown_testing::dev_testing_plugin::DevTestingPlugin::new();
  manual_integration_test!()
    .build(TestMeta { expect_executed: false, ..Default::default() })
    .run_dev_with_plugins(
      BundlerOptions {
        experimental: Some(ExperimentalOptions {
          hmr: Some(HmrOptions::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Arc::new(testing_plugin)],
      true,
      async |ctx| {
        ctx.mark_next_step();

        // Step 1: Edit hmr.js: 'hello' → 'hello1' + delay marker
        let content = std::fs::read_to_string(&hmr_js).unwrap();
        let new_content =
          content.replace("const foo = 'hello'", "const foo = 'hello1'\n// @delay-transform");
        std::fs::write(&hmr_js, new_content).unwrap();

        // Trigger rebuild - DON'T WAIT (starts 500ms delayed transform)
        ctx.dev_engine.notify_file_changes(std::iter::once(hmr_js.clone()).collect());

        // Step 2: Sleep 100ms (transform in progress, not complete)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Edit hmr.js: 'hello1' → 'hello2' (overlapping with first transform!)
        let content = std::fs::read_to_string(&hmr_js).unwrap();
        let new_content = content.replace("const foo = 'hello1'", "const foo = 'hello2'");
        std::fs::write(&hmr_js, new_content).unwrap();

        // Trigger rebuild and WAIT for final result
        ctx.dev_engine.notify_file_changes(std::iter::once(hmr_js.clone()).collect());

        ctx.dev_engine.ensure_empty_task_queue().await.unwrap();

        ctx.mark_next_step();

        // Step 3: Edit back to 'hello' (remove delay marker)
        let content = std::fs::read_to_string(&hmr_js).unwrap();
        let new_content =
          content.replace("const foo = 'hello2'\n// @delay-transform", "const foo = 'hello'");
        std::fs::write(&hmr_js, new_content).unwrap();

        ctx
          .dev_engine
          .ensure_task_with_changed_files(std::iter::once(hmr_js.clone()).collect())
          .await;

        ctx.dev_engine.ensure_empty_task_queue().await.unwrap();

        Ok(())
      },
    )
    .await;
}
