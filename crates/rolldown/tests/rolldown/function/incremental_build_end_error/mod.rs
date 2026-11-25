use std::{
  borrow::Cow,
  path::PathBuf,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
};

use arcstr::ArcStr;
use rolldown::{Bundler, BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem};
use rolldown_common::{Output, ScanMode, WatcherChangeKind};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};
use rolldown_plugin::{
  HookBuildEndArgs, HookBuildStartArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn,
  HookNoopReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
  HookTransformReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
  SharedTransformPluginContext,
};
use rolldown_std_utils::PathExt as _;
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::FxHashMap;

/// A build that transforms a module containing this string fails in `buildEnd`.
const FAIL_MARKER: &str = "FAIL_BUILD_END";

type Sources = Arc<Mutex<FxHashMap<ArcStr, &'static str>>>;

/// Serves the modules in `sources`, and fails `buildEnd` when the build transformed a module
/// containing `FAIL_MARKER`, like a plugin that collects problems while transforming and reports
/// them once the module graph is complete.
///
/// `buildEnd` runs after the scan has updated the incremental cache, and before `bundle_up` puts
/// back the symbol scoping the cache leaves out. The flag is reset on every `buildStart`, so the
/// error comes back only if the build fetches the module again.
#[derive(Debug)]
struct FailBuildEndOnMarker {
  cwd: PathBuf,
  sources: Sources,
  saw_marker: AtomicBool,
}

impl Plugin for FailBuildEndOnMarker {
  fn name(&self) -> Cow<'static, str> {
    "fail-build-end-on-marker".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
      | HookUsage::ResolveId
      | HookUsage::Load
      | HookUsage::Transform
      | HookUsage::BuildEnd
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    self.saw_marker.store(false, Ordering::Relaxed);
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let id: ArcStr = match args.specifier.strip_prefix("./") {
      Some(name) => self.cwd.join(name).expect_to_str().into(),
      None => args.specifier.into(),
    };
    let is_known = self.sources.lock().unwrap().contains_key(&id);
    Ok(is_known.then(|| HookResolveIdOutput::from_id(id)))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = self.sources.lock().unwrap().get(args.id).copied();
    Ok(code.map(|code| HookLoadOutput { code: code.into(), ..Default::default() }))
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.code.contains(FAIL_MARKER) {
      self.saw_marker.store(true, Ordering::Relaxed);
    }
    Ok(None)
  }

  async fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&HookBuildEndArgs<'_>>,
  ) -> HookNoopReturn {
    if self.saw_marker.load(Ordering::Relaxed) {
      Err(BatchedBuildDiagnostic::new(vec![anyhow::anyhow!("simulated buildEnd failure").into()]))?;
    }
    Ok(())
  }
}

/// `entry.js` imports `dep.js` and `other.js`.
struct Project {
  bundler: Bundler,
  sources: Sources,
  dep_id: ArcStr,
  other_id: ArcStr,
}

impl Project {
  fn new(dep: &'static str) -> Self {
    let cwd = std::env::current_dir().unwrap();
    let id = |name: &str| -> ArcStr { cwd.join(name).expect_to_str().into() };
    let entry_id = id("entry.js");
    let dep_id = id("dep.js");
    let other_id = id("other.js");
    let sources: Sources = Arc::new(Mutex::new(FxHashMap::from_iter([
      (
        entry_id.clone(),
        "import { dep } from './dep.js'; import { other } from './other.js'; console.log(dep, other);",
      ),
      (dep_id.clone(), dep),
      (other_id.clone(), "export const other = 'other';"),
    ])));
    let bundler = Bundler::with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: entry_id.to_string(),
        }]),
        experimental: Some(ExperimentalOptions {
          incremental_build: Some(true),
          dev_mode: Some(DevModeOptions::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![Plugin::new_shared(FailBuildEndOnMarker {
        cwd: cwd.clone(),
        sources: Arc::clone(&sources),
        saw_marker: AtomicBool::new(false),
      })],
    )
    .unwrap();
    Self { bundler, sources, dep_id, other_id }
  }

  fn edit(&self, id: &ArcStr, code: &'static str) {
    self.sources.lock().unwrap().insert(id.clone(), code);
  }

  /// The HMR update the dev engine computes for an edit before it rebuilds.
  async fn hot_update(&mut self, id: &ArcStr) -> BuildResult<()> {
    let changed = FxIndexMap::from_iter([(id.to_string(), WatcherChangeKind::Update)]);
    self.bundler.compute_hmr_update_for_file_changes(&changed, true, false).await?;
    Ok(())
  }

  async fn build(&mut self, changed: &[&ArcStr]) -> BuildResult<String> {
    let output = self
      .bundler
      .incremental_generate(ScanMode::Partial(changed.iter().map(|id| (*id).clone()).collect()))
      .await?;
    Ok(chunk_code(&output.assets))
  }
}

fn chunk_code(assets: &[Output]) -> String {
  let mut chunks = assets.iter().filter_map(|output| match output {
    Output::Chunk(chunk) => Some(chunk.code.clone()),
    Output::Asset(_) => None,
  });
  let code = chunks.next().expect("the build should emit a chunk");
  assert!(chunks.next().is_none(), "the build should emit a single chunk");
  code
}

fn assert_build_end_failure(result: BuildResult<String>) {
  let Err(errors) = result else {
    panic!("the build must fail in `buildEnd`");
  };
  assert!(
    errors
      .iter()
      .any(|error| error.to_diagnostic().to_string().contains("simulated buildEnd failure")),
    "unexpected errors: {:?}",
    errors.iter().map(|error| error.to_diagnostic().to_string()).collect::<Vec<_>>()
  );
}

/// A partial build that fails in `buildEnd`, after its scan updated the cache, must leave the
/// cache whole, and the next builds must fetch the failed build's modules again, so the error
/// keeps surfacing until they are fixed.
#[tokio::test(flavor = "multi_thread")]
async fn partial_build_keeps_failing_until_the_module_is_fixed() {
  let mut project = Project::new("export const dep = 'dep';");
  project.bundler.incremental_generate(ScanMode::Full).await.unwrap();
  let (dep_id, other_id) = (project.dep_id.clone(), project.other_id.clone());

  project.edit(&dep_id, "export const dep = 'FAIL_BUILD_END';");
  project.hot_update(&dep_id).await.unwrap();
  assert_build_end_failure(project.build(&[&dep_id]).await);

  // Only `other.js` changes: this build links on the cache the failed build left, and fetches
  // `dep.js` again.
  project.edit(&other_id, "export const other = 'edited';");
  project.hot_update(&other_id).await.unwrap();
  assert_build_end_failure(project.build(&[&other_id]).await);

  // A build whose scan fails leaves `dep.js` queued for the next build.
  project.edit(&other_id, "export const other = ;");
  project.hot_update(&other_id).await.unwrap_err();
  project.build(&[&other_id]).await.unwrap_err();
  project.edit(&other_id, "export const other = 'edited again';");
  project.hot_update(&other_id).await.unwrap();
  assert_build_end_failure(project.build(&[&other_id]).await);

  project.edit(&dep_id, "export const dep = 'fixed';");
  project.hot_update(&dep_id).await.unwrap();
  let code = project.build(&[&dep_id]).await.unwrap();
  assert!(code.contains("\"fixed\"") && code.contains("\"edited again\""), "{code}");
}

/// A full build that fails in `buildEnd` leaves no graph to build on, as a failed full scan does,
/// so the next build is a full one.
#[tokio::test(flavor = "multi_thread")]
async fn full_build_keeps_failing_until_the_module_is_fixed() {
  let mut project = Project::new("export const dep = 'FAIL_BUILD_END';");
  let Err(errors) = project.bundler.incremental_generate(ScanMode::Full).await else {
    panic!("the initial build must fail in `buildEnd`");
  };
  assert!(!errors.is_empty());
  let (dep_id, other_id) = (project.dep_id.clone(), project.other_id.clone());

  // The failed build left no graph: the HMR update finds no module to update, and the build runs
  // as a full one.
  project.edit(&other_id, "export const other = 'edited';");
  project.hot_update(&other_id).await.unwrap();
  assert_build_end_failure(project.build(&[&other_id]).await);

  project.edit(&dep_id, "export const dep = 'fixed';");
  project.hot_update(&dep_id).await.unwrap();
  let code = project.build(&[&dep_id]).await.unwrap();
  assert!(code.contains("\"fixed\"") && code.contains("\"edited\""), "{code}");
}
