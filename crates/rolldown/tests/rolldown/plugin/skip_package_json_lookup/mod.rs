use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use rolldown::{BundlerOptions, InputItem};
use rolldown_plugin::{
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

/// Keeps `./target.js` without `package.json` metadata, as the Vite resolver does for
/// `legacyInconsistentCjsInterop`.
#[derive(Debug)]
struct OptOut;

impl Plugin for OptOut {
  fn name(&self) -> Cow<'static, str> {
    "opt-out".into()
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier != "@target" {
      return Ok(None);
    }
    let resolved =
      ctx.resolve("./target.js", args.importer, None).await?.expect("the target should resolve");
    Ok(Some(HookResolveIdOutput {
      id: resolved.id.as_arc_str().clone(),
      skip_package_json_lookup: true,
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

/// Forwards `ctx.resolve`'s answer as its own, the way `viteAliasPlugin` does, and records
/// whether the opt-out came through both conversions.
#[derive(Debug)]
struct Forwarder {
  kept: Arc<AtomicBool>,
}

impl Plugin for Forwarder {
  fn name(&self) -> Cow<'static, str> {
    "forwarder".into()
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier != "@forwarded" {
      return Ok(None);
    }
    let resolved =
      ctx.resolve("@target", args.importer, None).await?.expect("the target should resolve");
    let output = HookResolveIdOutput::from_resolved_id(resolved.clone());
    self.kept.store(
      resolved.skip_package_json_lookup && output.skip_package_json_lookup,
      Ordering::Relaxed,
    );
    Ok(Some(output))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

/// The opt-out has to reach the `ResolvedId` that `ctx.resolve` returns, and survive the way
/// back into a hook output. Otherwise a hook that forwards another hook's answer would lose it.
#[tokio::test(flavor = "multi_thread")]
async fn skip_package_json_lookup_survives_forwarding() {
  let kept = Arc::new(AtomicBool::new(false));
  manual_integration_test!()
    .build(TestMeta { snapshot: false, expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(Forwarder { kept: Arc::clone(&kept) }), Plugin::new_shared(OptOut)],
    )
    .await;

  assert!(
    kept.load(Ordering::Relaxed),
    "the opt-out must survive `ctx.resolve` and `HookResolveIdOutput::from_resolved_id`"
  );
}
