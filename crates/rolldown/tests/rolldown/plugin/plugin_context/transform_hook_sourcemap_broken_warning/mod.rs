use std::{borrow::Cow, sync::Arc};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin};

/// A plugin whose `transform` hook returns code without providing a sourcemap.
/// `output_map` controls whether the result has `map: Omitted` (which triggers
/// `SOURCEMAP_BROKEN` when sourcemap output is enabled) or `map: Null` (which
/// always suppresses the warning). `mutate` controls whether the returned code
/// differs from the input — `SOURCEMAP_BROKEN` fires either way, since Rollup
/// keys off whether a map was provided, not whether the code changed.
#[derive(Debug)]
struct NoMapTransformPlugin {
  output_map: fn() -> HookTransformOutputMap,
  mutate: bool,
}

impl Plugin for NoMapTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    "no-map-transform".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    // Only transform the entry module, not rolldown's internal runtime module,
    // so each test produces exactly one `SOURCEMAP_BROKEN` warning.
    if !args.id.ends_with("entry.js") {
      return Ok(None);
    }
    let code = if self.mutate {
      args.code.replace("'hello world'", "'hello from plugin'")
    } else {
      args.code.to_string()
    };
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(code),
      map: (self.output_map)(),
      ..Default::default()
    }))
  }
}

/// A captured `SOURCEMAP_BROKEN` warning: `(plugin, message, id)`.
type CapturedWarning = (Option<String>, String, Option<String>);

/// Builds `entry.js` with `NoMapTransformPlugin` and returns the `SOURCEMAP_BROKEN`
/// warnings collected into `BundleOutput::warnings`. `sourcemap` controls whether
/// sourcemap output is enabled; `mutate` controls whether the plugin's returned
/// code differs from the input.
async fn run(
  output_map: fn() -> HookTransformOutputMap,
  sourcemap: Option<SourceMapType>,
  mutate: bool,
) -> Vec<CapturedWarning> {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(
        concat!(
          env!("CARGO_MANIFEST_DIR"),
          "/tests/rolldown/plugin/plugin_context/transform_hook_sourcemap_broken_warning"
        )
        .into(),
      ),
      sourcemap,
      ..Default::default()
    },
    vec![Arc::new(NoMapTransformPlugin { output_map, mutate })],
  )
  .expect("failed to create bundler");

  let output = bundler.generate().await.expect("build should succeed");
  output
    .warnings
    .iter()
    .filter(|warning| warning.kind().to_string() == "SOURCEMAP_BROKEN")
    .map(|warning| (warning.plugin(), warning.to_string(), warning.id()))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn omitted_map_emits_sourcemap_broken_warning() {
  let warnings =
    Box::pin(run(|| HookTransformOutputMap::Omitted, Some(SourceMapType::File), true)).await;
  assert_eq!(warnings.len(), 1, "exactly one SOURCEMAP_BROKEN warning must be emitted");
  let (plugin, message, id) = &warnings[0];
  assert_eq!(plugin.as_deref(), Some("no-map-transform"));
  assert!(
    message.contains("didn't generate a sourcemap"),
    "warning message should mention the missing sourcemap, got: {message}"
  );
  // The warning carries the id of the transformed module.
  assert!(
    id.as_deref().is_some_and(|id| id.ends_with("entry.js")),
    "SOURCEMAP_BROKEN warning should carry the module id, got: {id:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn omitted_map_emits_warning_even_when_code_unchanged() {
  // Rollup emits `SOURCEMAP_BROKEN` whenever a transform hook returns code without
  // a sourcemap, even if the returned code is identical to the input (see Rollup's
  // `transform-without-sourcemap-render-chunk` fixture).
  let warnings =
    Box::pin(run(|| HookTransformOutputMap::Omitted, Some(SourceMapType::File), false)).await;
  assert_eq!(
    warnings.len(),
    1,
    "SOURCEMAP_BROKEN warning must be emitted when map is Omitted, even if code is unchanged"
  );
  assert_eq!(warnings[0].0.as_deref(), Some("no-map-transform"));
}

#[tokio::test(flavor = "multi_thread")]
async fn null_map_suppresses_sourcemap_broken_warning() {
  let warnings =
    Box::pin(run(|| HookTransformOutputMap::Null, Some(SourceMapType::File), true)).await;
  assert!(warnings.is_empty(), "SOURCEMAP_BROKEN warning must not be emitted when map is Null");
}

#[tokio::test(flavor = "multi_thread")]
async fn omitted_map_without_sourcemap_suppresses_warning() {
  let warnings = Box::pin(run(|| HookTransformOutputMap::Omitted, None, true)).await;
  assert!(
    warnings.is_empty(),
    "SOURCEMAP_BROKEN warning must not be emitted when sourcemap output is disabled, \
     mirroring Rollup which only inspects the sourcemap chain while collapsing sourcemaps"
  );
}
