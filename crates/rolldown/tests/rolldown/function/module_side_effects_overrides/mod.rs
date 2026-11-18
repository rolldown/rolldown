use std::{
  borrow::Cow,
  path::PathBuf,
  sync::{Arc, Mutex},
};

use arcstr::ArcStr;
use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::{
  InnerOptions, ModuleSideEffects, Output, TreeshakeOptions,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};
use rolldown_error::BuildResult;
use rolldown_plugin::{
  HookBuildEndArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn, HookNoopReturn,
  HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
  HookTransformOutput, HookTransformReturn, HookUsage, Plugin, PluginContext,
  SharedLoadPluginContext, SharedTransformPluginContext,
};
use rolldown_std_utils::PathExt as _;

const ENTRY: &str = "import './load-then-transform.js'; import './build-end.js'; import './null-write.js'; console.log('ENTRY');";

/// `PluginContext::set_module_side_effects` is the store behind the JS setter of
/// `getModuleInfo(id).moduleSideEffects`.
#[derive(Debug)]
struct Writer {
  cwd: PathBuf,
}

impl Writer {
  fn id(&self, name: &str) -> ArcStr {
    self.cwd.join(name).expect_to_str().into()
  }
}

impl Plugin for Writer {
  fn name(&self) -> Cow<'static, str> {
    "writer".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform | HookUsage::BuildEnd
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let name = args.specifier.strip_prefix("./").unwrap_or(args.specifier);
    let mut output = HookResolveIdOutput::from_id(self.id(name));
    if name == "null-write.js" {
      output.side_effects = Some(HookSideEffects::False);
    }
    Ok(Some(output))
  }

  async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let name = args.id.strip_prefix(self.cwd.expect_to_str()).unwrap_or(args.id);
    let (code, side_effects) = match name.trim_start_matches(['/', '\\']) {
      "entry.js" => (ENTRY, None),
      "load-then-transform.js" => {
        // The `transform` hook below returns a value after this write, so its value counts.
        ctx.set_module_side_effects(args.id, Some(HookSideEffects::NoTreeshake));
        ("console.log('LOAD_THEN_TRANSFORM');", None)
      }
      "build-end.js" => ("console.log('BUILD_END');", Some(HookSideEffects::False)),
      "null-write.js" => ("console.log('NULL_WRITE');", None),
      _ => return Ok(None),
    };
    Ok(Some(HookLoadOutput { code: code.into(), side_effects, ..Default::default() }))
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id != self.id("load-then-transform.js").as_str() {
      return Ok(None);
    }
    Ok(Some(HookTransformOutput {
      side_effects: Some(HookSideEffects::False),
      ..Default::default()
    }))
  }

  async fn build_end(
    &self,
    ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    ctx.set_module_side_effects(&self.id("build-end.js"), Some(HookSideEffects::NoTreeshake));
    // `null`: "not decided". The `resolveId` value no longer counts.
    ctx.set_module_side_effects(&self.id("null-write.js"), None);
    Ok(())
  }
}

/// Per module: the shown value of `get_module_info`, and the value of `get_module_side_effects`.
type Seen = Arc<Mutex<Vec<(&'static str, String, String)>>>;

/// Reads the values after `Writer` wrote them, as a Rust plugin sees them.
#[derive(Debug)]
struct Reader {
  cwd: PathBuf,
  seen: Seen,
}

impl Plugin for Reader {
  fn name(&self) -> Cow<'static, str> {
    "reader".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildEnd
  }

  async fn build_end(
    &self,
    ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    for name in ["load-then-transform.js", "build-end.js", "null-write.js"] {
      let id = self.cwd.join(name);
      let id = id.expect_to_str();
      let from_getter = show(ctx.get_module_side_effects(id).await?);
      let from_info = ctx.get_module_info(id).and_then(|info| info.side_effects?.shown());
      self.seen.lock().unwrap().push((name, show(from_info), from_getter));
    }
    Ok(())
  }
}

/// `DeterminedSideEffects` has no `PartialEq`.
fn show(side_effects: Option<DeterminedSideEffects>) -> String {
  format!("{side_effects:?}")
}

/// `treeshake.moduleSideEffects` as a function that records each call and decides nothing.
fn counting_option(calls: &Arc<Mutex<Vec<String>>>) -> TreeshakeOptions {
  let calls = Arc::clone(calls);
  TreeshakeOptions::Option(InnerOptions {
    module_side_effects: ModuleSideEffects::Function(Arc::new(move |id: &str, _external: bool| {
      let name = id.rsplit(['/', '\\']).next().unwrap_or(id).to_string();
      calls.lock().unwrap().push(name);
      Box::pin(async { Ok(None) })
    })),
    ..Default::default()
  })
}

async fn build(
  treeshake: Option<TreeshakeOptions>,
) -> BuildResult<(String, Vec<(&'static str, String)>)> {
  let cwd = std::env::current_dir().unwrap();
  let seen = Arc::new(Mutex::new(vec![]));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: cwd.join("entry.js").expect_to_str().to_string(),
      }]),
      treeshake: treeshake.unwrap_or_default(),
      ..Default::default()
    },
    vec![
      Plugin::new_shared(Writer { cwd: cwd.clone() }),
      Plugin::new_shared(Reader { cwd, seen: Arc::clone(&seen) }),
    ],
  )
  .unwrap();
  let output = bundler.generate().await?;
  let code = output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .expect("the build should emit a chunk");
  let seen = seen
    .lock()
    .unwrap()
    .iter()
    .map(|(name, from_info, from_getter)| {
      assert_eq!(from_info, from_getter, "`{name}`: the info and the getter disagree");
      (*name, from_info.clone())
    })
    .collect();
  Ok((code, seen))
}

fn expected_seen() -> Vec<(&'static str, String)> {
  vec![
    // A Rust `transform` value after a write replaces the written value.
    ("load-then-transform.js", show(Some(DeterminedSideEffects::UserDefined(false)))),
    // A write in `buildEnd` is visible at once, and replaces the `load` value.
    ("build-end.js", show(Some(DeterminedSideEffects::NoTreeshake))),
    // After a `null` write, the value without hooks shows.
    ("null-write.js", show(Some(DeterminedSideEffects::UserDefined(true)))),
  ]
}

#[tokio::test(flavor = "multi_thread")]
async fn writes_reach_rust_plugins_and_tree_shaking() {
  let (code, seen) = build(None).await.unwrap();
  assert_eq!(seen, expected_seen());
  assert!(code.contains("ENTRY"), "{code}");
  assert!(!code.contains("LOAD_THEN_TRANSFORM"), "{code}");
  assert!(code.contains("BUILD_END"), "{code}");
  assert!(code.contains("NULL_WRITE"), "{code}");
}

/// The option function runs only for a module whose value no hook decided (`entry.js`), and for
/// the module that gets a `null` write, which a Rust reader then reads. Each runs once.
#[tokio::test(flavor = "multi_thread")]
async fn option_function_runs_only_when_needed() {
  let calls = Arc::new(Mutex::new(vec![]));
  let (code, seen) = build(Some(counting_option(&calls))).await.unwrap();
  assert_eq!(seen, expected_seen());
  assert!(code.contains("NULL_WRITE"), "{code}");
  let mut calls = calls.lock().unwrap().clone();
  calls.sort();
  assert_eq!(calls, ["entry.js", "null-write.js"]);
}
