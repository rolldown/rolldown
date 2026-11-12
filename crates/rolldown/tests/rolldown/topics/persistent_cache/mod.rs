//! The persistent scan cache across restarts. Each build uses a new `Bundler`, as a new process
//! would, and reads the cache from disk.

use std::{
  borrow::Cow,
  fmt::Write as _,
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
};

use rolldown::{
  BundleOutput, Bundler, BundlerOptions, CacheOptions, ExperimentalOptions, InputItem, LogLevel,
  ModuleType, OnLog,
  plugin::{
    __inner::SharedPluginable, HookFilterArgs, HookLoadArgs, HookLoadOutput, HookLoadReturn,
    HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookTransformArgs,
    HookTransformOutput, HookTransformReturn, HookUsage, LogWithoutPlugin, Plugin, PluginContext,
    PluginHookMeta, SharedLoadPluginContext, SharedTransformPluginContext,
  },
};
use rolldown_common::{EmittedAsset, ImportAttributes, ModuleDefFormat, StrOrBytes};

/// A project in a temporary directory. The cache lives in `<dir>/.cache`.
struct Project {
  dir: PathBuf,
}

impl Drop for Project {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.dir);
  }
}

impl Project {
  fn new(files: &[(&str, &str)]) -> Self {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
      std::env::temp_dir().join(format!("rolldown-persistent-cache-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let project = Self { dir };
    for (path, content) in files {
      project.write(path, content);
    }
    project
  }

  fn path(&self, relative: &str) -> PathBuf {
    self.dir.join(relative)
  }

  fn id(&self, relative: &str) -> String {
    // The temp dir can be behind a symbolic link (macOS `/var` -> `/private/var`). Module ids
    // are real paths.
    dunce::canonicalize(&self.dir).unwrap().join(relative).to_string_lossy().into_owned()
  }

  fn write(&self, relative: &str, content: &str) {
    let path = self.path(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
  }

  fn remove(&self, relative: &str) {
    std::fs::remove_file(self.path(relative)).unwrap();
  }

  fn cache_dir(&self) -> PathBuf {
    self.path(".cache")
  }

  fn options(&self, cache: Option<CacheOptions>) -> BundlerOptions {
    BundlerOptions {
      cwd: Some(dunce::canonicalize(&self.dir).unwrap()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      experimental: Some(ExperimentalOptions { cache, ..Default::default() }),
      ..Default::default()
    }
  }

  fn cache(&self) -> CacheOptions {
    CacheOptions { dir: ".cache".into(), ..Default::default() }
  }
}

#[derive(Debug, Default)]
struct Calls(Mutex<Vec<String>>);

impl Calls {
  fn push(&self, call: String) {
    self.0.lock().unwrap().push(call);
  }

  fn take(&self) -> Vec<String> {
    let mut calls = std::mem::take(&mut *self.0.lock().unwrap());
    calls.sort();
    calls
  }
}

/// Records every `resolveId`, `load` and `transform` call on a module of the project. The
/// transform appends a comment, so the output shows that the hook ran or was reused.
#[derive(Debug, Clone)]
struct Recorder {
  name: &'static str,
  cache: Option<bool>,
  calls: Arc<Calls>,
  /// Only these files (by file name) pass the hook filter. `None` lets every module pass.
  filter: Option<Vec<&'static str>>,
  /// Files whose transform reads a dependency with `this.addWatchFile`.
  watch: Vec<(&'static str, PathBuf)>,
  /// Files whose transform prints a warning.
  warn: Vec<&'static str>,
  /// Files whose transform emits an asset.
  emit: Vec<&'static str>,
  /// Files whose transform calls `this.load` on `./b.js`.
  load: Vec<&'static str>,
}

impl Recorder {
  fn new(name: &'static str, cache: Option<bool>) -> (Arc<Self>, Arc<Calls>) {
    let calls = Arc::new(Calls::default());
    let plugin = Self {
      name,
      cache,
      calls: Arc::clone(&calls),
      filter: None,
      watch: vec![],
      warn: vec![],
      emit: vec![],
      load: vec![],
    };
    (Arc::new(plugin), calls)
  }

  fn file_name(id: &str) -> &str {
    Path::new(id).file_name().and_then(|n| n.to_str()).unwrap_or(id)
  }

  fn matches(&self, id: &str) -> bool {
    self.filter.as_ref().is_none_or(|files| files.contains(&Self::file_name(id)))
  }
}

impl Plugin for Recorder {
  fn name(&self) -> Cow<'static, str> {
    self.name.into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
  }

  fn hook_filter_matches(&self, _ctx: &PluginContext, args: HookFilterArgs<'_>) -> bool {
    match args {
      // With a file filter, `resolveId` never matches.
      HookFilterArgs::ResolveId(_) => self.filter.is_none(),
      HookFilterArgs::Load(args) => self.matches(args.id),
      HookFilterArgs::Transform(args) => self.matches(args.id),
      HookFilterArgs::RenderChunk(_) => true,
    }
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if !self.hook_filter_matches(ctx, HookFilterArgs::ResolveId(args)) {
      return Ok(None);
    }
    if args.importer.is_some() {
      self.calls.push(format!("resolveId {}", args.specifier));
    }
    // `@/x` points into `src/`, as a tsconfig alias would. It shows that a reused saved
    // result still resolves the import.
    if let Some(rest) = args.specifier.strip_prefix("@/") {
      let cwd = ctx.cwd();
      return Ok(Some(HookResolveIdOutput {
        id: cwd.join("src").join(rest).to_string_lossy().into_owned().into(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: self.cache })
  }

  async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if !self.hook_filter_matches(&ctx, HookFilterArgs::Load(args)) {
      return Ok(None);
    }
    if !args.id.starts_with('\0') {
      self.calls.push(format!("load {}", Self::file_name(args.id)));
    }
    Ok(None)
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: self.cache })
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !self.hook_filter_matches(&ctx, HookFilterArgs::Transform(args)) || args.id.starts_with('\0')
    {
      return Ok(None);
    }
    let file = Self::file_name(args.id);
    self.calls.push(format!("transform {file}"));
    let mut code = format!("{}\n// {} saw {file}\n", args.code, self.name);
    for (watched, dependency) in &self.watch {
      if *watched == file {
        ctx.add_watch_file(&dependency.to_string_lossy());
        let content = std::fs::read_to_string(dependency).unwrap_or_else(|_| "missing".into());
        let _ = writeln!(code, "console.log({content:?});");
      }
    }
    if self.warn.contains(&file) {
      ctx.warn(LogWithoutPlugin {
        message: format!("warning from {file}"),
        id: Some(args.id.to_string()),
        code: Some("TEST_WARNING".into()),
        ..Default::default()
      });
    }
    if self.emit.contains(&file) {
      let reference_id = ctx.emit_file(
        EmittedAsset {
          name: Some(format!("{file}.txt")),
          source: StrOrBytes::Str(format!("asset of {file}")),
          ..Default::default()
        },
        None,
        None,
      )?;
      let _ = writeln!(code, "export const asset = {reference_id:?};");
    }
    if self.load.contains(&file) {
      let b = Path::new(args.id).with_file_name("b.js");
      ctx
        .load(&b.to_string_lossy(), ImportAttributes::default(), None, ModuleDefFormat::default())
        .await?;
    }
    Ok(Some(HookTransformOutput { code: Some(code), ..Default::default() }))
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: self.cache })
  }
}

struct BuildResult {
  /// File name and content of each output file, sorted by file name.
  files: Vec<(String, String)>,
  warnings: Vec<String>,
  logs: Vec<String>,
}

/// Collects the logs of a build in `logs`.
fn with_log(options: BundlerOptions, logs: &Arc<Mutex<Vec<String>>>) -> BundlerOptions {
  BundlerOptions {
    on_log: Some(OnLog::new(Arc::new({
      let logs = Arc::clone(logs);
      move |level: LogLevel, log| {
        logs.lock().unwrap().push(format!(
          "{level:?} {} {} {}",
          log.plugin.unwrap_or_default(),
          log.code.unwrap_or_default(),
          log.message
        ));
        Box::pin(async { Ok(()) })
      }
    }))),
    ..options
  }
}

async fn build(options: BundlerOptions, plugins: Vec<SharedPluginable>) -> BuildResult {
  let logs = Arc::new(Mutex::new(vec![]));
  let options = with_log(options, &logs);
  let mut bundler = Bundler::with_plugins(options, plugins).expect("create bundler");
  let BundleOutput { assets, warnings, .. } =
    bundler.generate().await.expect("build should succeed");
  bundler.close().await.expect("close");
  let mut files: Vec<(String, String)> = assets
    .iter()
    .map(|output| {
      (
        output.filename().to_string(),
        String::from_utf8_lossy(output.content_as_bytes()).into_owned(),
      )
    })
    .collect();
  files.sort();
  let mut logs = std::mem::take(&mut *logs.lock().unwrap());
  logs.sort();
  BuildResult {
    files,
    warnings: warnings.iter().map(|w| w.to_diagnostic().to_string()).collect(),
    logs,
  }
}

fn plugins(list: &[&Arc<Recorder>]) -> Vec<SharedPluginable> {
  list.iter().map(|plugin| <Recorder as Plugin>::new_shared((***plugin).clone())).collect()
}

fn files_in(dir: &Path) -> usize {
  std::fs::read_dir(dir).map(Iterator::count).unwrap_or(0)
}

fn basic_project() -> Project {
  Project::new(&[
    ("main.js", "import { b } from './b.js';\nimport { c } from '@/c.js';\nconsole.log(b, c);\n"),
    ("b.js", "export const b = 'b';\n"),
    ("src/c.js", "export const c = 'c';\n"),
  ])
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_reuses_saved_results_and_gives_the_same_output() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));

  let clean = build(project.options(None), plugins(&[&plugin])).await;
  calls.take();

  let first = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(
    calls.take(),
    [
      "load b.js",
      "load c.js",
      "load main.js",
      "resolveId ./b.js",
      "resolveId @/c.js",
      "transform b.js",
      "transform c.js",
      "transform main.js",
    ]
  );
  assert!(files_in(&project.cache_dir()) > 0, "the cache is written");

  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new(), "no hook of a saved module runs again");
  assert_eq!(second.files, first.files);
  assert_eq!(second.files, clean.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_off_writes_nothing() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  build(project.options(None), plugins(&[&plugin])).await;
  build(project.options(None), plugins(&[&plugin])).await;
  assert!(!project.cache_dir().exists());
  assert_eq!(calls.take().len(), 16, "every hook runs in both builds");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hook_without_cache_runs_again_for_the_modules_it_was_called_for() {
  let project = basic_project();
  let (cached, cached_calls) = Recorder::new("cached", Some(true));
  // This plugin's hooks only see `b.js`, and they do not have the cache on.
  let (mut uncached, uncached_calls) = Recorder::new("uncached", None);
  Arc::get_mut(&mut uncached).unwrap().filter = Some(vec!["b.js"]);

  let first = build(project.options(Some(project.cache())), plugins(&[&cached, &uncached])).await;
  cached_calls.take();
  uncached_calls.take();

  let second = build(project.options(Some(project.cache())), plugins(&[&cached, &uncached])).await;
  // `b.js` has no saved result, because hooks without the cache were called for it. The filter
  // blocks every other call of the uncached plugin, so the other modules are reused.
  assert_eq!(cached_calls.take(), ["load b.js", "transform b.js"]);
  assert_eq!(uncached_calls.take(), ["load b.js", "transform b.js"]);
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_false_is_not_changed_by_assume_cacheable_hooks() {
  let project = basic_project();
  let (unset, unset_calls) = Recorder::new("unset", None);
  let cache = CacheOptions {
    assume_cacheable_hooks: Some(rolldown::AssumeCacheableHooks {
      resolve_id: Some(true),
      load: Some(true),
      transform: Some(true),
    }),
    ..project.cache()
  };
  build(project.options(Some(cache.clone())), plugins(&[&unset])).await;
  unset_calls.take();
  build(project.options(Some(cache.clone())), plugins(&[&unset])).await;
  assert_eq!(unset_calls.take(), Vec::<String>::new(), "assumed cacheable");

  let (never, never_calls) = Recorder::new("never", Some(false));
  build(project.options(Some(cache.clone())), plugins(&[&never])).await;
  never_calls.take();
  build(project.options(Some(cache)), plugins(&[&never])).await;
  assert_eq!(never_calls.take().len(), 8, "`cache: false` always runs");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_module_runs_again_alone() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();

  project.write("b.js", "export const b = 'changed';\n");
  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), ["load b.js", "transform b.js"]);
  let clean = build(project.options(None), plugins(&[&plugin])).await;
  assert_eq!(second.files, clean.files);
  assert!(second.files[0].1.contains("changed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_transform_dependency_makes_its_module_run_again() {
  let project = basic_project();
  project.write("theme.json", "red");
  let (mut plugin, calls) = Recorder::new("p", Some(true));
  Arc::get_mut(&mut plugin).unwrap().watch = vec![("b.js", project.path("theme.json"))];

  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();

  // Unchanged: reused.
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());

  // Changed contents.
  project.write("theme.json", "blue");
  let changed = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), ["load b.js", "transform b.js"]);
  assert!(changed.files[0].1.contains("blue"));

  // Deleted.
  project.remove("theme.json");
  let deleted = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), ["load b.js", "transform b.js"]);
  assert!(deleted.files[0].1.contains("missing"));

  // Created again with the first contents: the first saved result is found again.
  project.write("theme.json", "red");
  let restored = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new(), "the result of the first build is reused");
  assert!(restored.files[0].1.contains("red"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_previously_missing_dependency_that_appears_makes_its_module_run_again() {
  let project = basic_project();
  let (mut plugin, calls) = Recorder::new("p", Some(true));
  Arc::get_mut(&mut plugin).unwrap().watch = vec![("b.js", project.path("theme.json"))];
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();

  project.write("theme.json", "new");
  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), ["load b.js", "transform b.js"]);
  assert!(second.files[0].1.contains("new"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_tsconfig_makes_its_modules_run_again() {
  let project = basic_project();
  // Without `allowJs`, a tsconfig does not govern `.js` files.
  project
    .write("tsconfig.json", r#"{ "compilerOptions": { "allowJs": true, "target": "es2020" } }"#);
  let (plugin, calls) = Recorder::new("p", Some(true));
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();

  project
    .write("tsconfig.json", r#"{ "compilerOptions": { "allowJs": true, "target": "es2022" } }"#);
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8, "every module under the tsconfig runs again");
}

#[tokio::test(flavor = "multi_thread")]
async fn options_cache_key_and_build_dependencies_change_every_key() {
  let project = basic_project();
  project.write("rolldown.config.js", "export default {}");
  let (plugin, calls) = Recorder::new("p", Some(true));
  let cache = CacheOptions {
    cache_key: Some("a".into()),
    build_dependencies: Some(vec!["rolldown.config.js".into()]),
    ..project.cache()
  };
  build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  calls.take();

  build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 0);

  // A changed option.
  let mut options = project.options(Some(cache.clone()));
  options.define = Some(vec![("__X__".into(), "1".into())].into_iter().collect());
  build(options, plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8, "a changed option is a miss for every module");

  // A changed cache key.
  let cache_b = CacheOptions { cache_key: Some("b".into()), ..cache.clone() };
  build(project.options(Some(cache_b)), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8, "a changed cache key is a miss for every module");

  // A changed build dependency.
  project.write("rolldown.config.js", "export default { changed: true }");
  build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8, "a changed build dependency is a miss for every module");

  // Changing `dir`-unrelated cache options does not matter: `maxAge` is not an input.
  let cache_max_age = CacheOptions { max_age: Some(1e9), ..cache };
  build(project.options(Some(cache_max_age)), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 0, "maxAge is not part of the options hash");
}

#[tokio::test(flavor = "multi_thread")]
async fn assume_unchanged_skips_hashing_and_relies_on_build_dependencies() {
  let project = Project::new(&[
    ("main.js", "import { lib } from './node_modules/lib/index.js';\nconsole.log(lib);\n"),
    ("node_modules/lib/index.js", "export const lib = 1;\n"),
    ("lock.yaml", "v1"),
  ]);
  let (plugin, calls) = Recorder::new("p", Some(true));
  let cache = CacheOptions {
    build_dependencies: Some(vec!["lock.yaml".into()]),
    assume_unchanged: Some(vec![rolldown_utils::pattern_filter::StringOrRegex::String(
      "**/node_modules/**".into(),
    )]),
    ..project.cache()
  };
  build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  calls.take();

  // The host broke its guarantee: the package changed but the lock file did not. The file is
  // assumed unchanged, so its saved result is reused.
  project.write("node_modules/lib/index.js", "export const lib = 2;\n");
  build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());

  // With the lock file changed, every module runs again.
  project.write("lock.yaml", "v2");
  let third = build(project.options(Some(cache)), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 5);
  let clean = build(project.options(None), plugins(&[&plugin])).await;
  assert_eq!(third.files, clean.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_and_emitted_files_are_done_again_on_reuse() {
  let project = basic_project();
  let (mut plugin, calls) = Recorder::new("p", Some(true));
  {
    let plugin = Arc::get_mut(&mut plugin).unwrap();
    plugin.warn = vec!["b.js"];
    plugin.emit = vec!["b.js"];
  }
  let first = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();
  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());
  assert_eq!(second.files, first.files);
  assert!(
    second.files.iter().any(|(name, content)| name.ends_with(".txt") && content == "asset of b.js")
  );
  assert_eq!(second.logs, first.logs);
  assert!(second.logs.iter().any(|log| log.contains("TEST_WARNING warning from b.js")));
}

#[tokio::test(flavor = "multi_thread")]
async fn this_load_makes_the_calling_module_run_again() {
  let project = basic_project();
  let (mut plugin, calls) = Recorder::new("p", Some(true));
  Arc::get_mut(&mut plugin).unwrap().load = vec!["main.js"];
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(
    calls.take(),
    ["load main.js", "resolveId ./b.js", "resolveId @/c.js", "transform main.js"]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn unresolved_imports_warn_again_on_reuse() {
  let project = Project::new(&[("main.js", "import 'missing-package';\nconsole.log(1);\n")]);
  let (plugin, calls) = Recorder::new("p", Some(true));
  let first = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();
  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());
  assert_eq!(second.warnings, first.warnings);
  assert!(second.warnings.iter().any(|w| w.contains("missing-package")));
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn data_url_imports_only_affect_their_importer() {
  let project = Project::new(&[
    ("main.js", "import './a.js';\nimport { b } from './b.js';\nconsole.log(b);\n"),
    ("a.js", "import 'data:text/javascript,console.log(1)';\n"),
    ("b.js", "export const b = 1;\n"),
  ]);
  let (plugin, calls) = Recorder::new("p", Some(true));
  let first = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();
  let second = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  // `a.js` imports a data URL. The data URL plugin's `resolveId` is not cacheable, so `a.js`
  // runs again. The other modules are reused.
  assert_eq!(
    calls.take(),
    ["load a.js", "resolveId data:text/javascript,console.log(1)", "transform a.js"]
  );
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn damaged_or_removed_cache_data_does_not_break_the_build() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  let first = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  calls.take();

  for entry in std::fs::read_dir(project.cache_dir()).unwrap() {
    let path = entry.unwrap().path();
    let mut bytes = std::fs::read(&path).unwrap();
    for byte in bytes.iter_mut().skip(16).step_by(7) {
      *byte ^= 0x5a;
    }
    std::fs::write(&path, bytes).unwrap();
  }
  let damaged = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(damaged.files, first.files);
  calls.take();

  std::fs::remove_dir_all(project.cache_dir()).unwrap();
  let removed = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8);
  assert_eq!(removed.files, first.files);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_write_warns_and_does_not_fail_the_build() {
  use std::os::unix::fs::PermissionsExt;
  let project = basic_project();
  std::fs::create_dir_all(project.cache_dir()).unwrap();
  std::fs::set_permissions(project.cache_dir(), std::fs::Permissions::from_mode(0o555)).unwrap();
  if std::fs::write(project.cache_dir().join("probe"), "").is_ok() {
    // Running as root: the directory is still writable.
    return;
  }
  let (plugin, _calls) = Recorder::new("p", Some(true));
  let result = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert!(result.logs.iter().any(|log| log.contains("PERSISTENT_CACHE_WRITE_FAILED")));
  std::fs::set_permissions(project.cache_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn two_builds_can_share_one_cache_dir_at_the_same_time() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  let (a, b) = tokio::join!(
    build(project.options(Some(project.cache())), plugins(&[&plugin])),
    build(project.options(Some(project.cache())), plugins(&[&plugin])),
  );
  assert_eq!(a.files, b.files);
  calls.take();
  let third = build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());
  assert_eq!(third.files, a.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn native_magic_string_with_sourcemaps_turns_the_cache_off() {
  let project = basic_project();
  let (plugin, _calls) = Recorder::new("p", Some(true));
  let mut options = project.options(Some(project.cache()));
  options.sourcemap = Some(rolldown::SourceMapType::File);
  options.experimental.as_mut().unwrap().native_magic_string = Some(true);
  build(options, plugins(&[&plugin])).await;
  assert!(!project.cache_dir().exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_incremental_build_saves_its_results_for_the_next_restart() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  let mut options = project.options(Some(project.cache()));
  options.experimental.as_mut().unwrap().incremental_build = Some(true);

  let logs = Arc::new(Mutex::new(vec![]));
  let mut bundler =
    Bundler::with_plugins(with_log(options.clone(), &logs), plugins(&[&plugin])).unwrap();
  bundler.incremental_generate(rolldown_common::ScanMode::Full).await.unwrap();
  calls.take();
  project.write("b.js", "export const b = 'edited';\n");
  bundler
    .incremental_generate(rolldown_common::ScanMode::Partial(vec![project.id("b.js").into()]))
    .await
    .unwrap();
  assert_eq!(
    calls.take(),
    ["load b.js", "transform b.js"],
    "an incremental build does not read the cache"
  );
  bundler.close().await.unwrap();
  drop(bundler);

  let restarted = build(options, plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new(), "the edit was saved by the incremental build");
  assert!(restarted.files[0].1.contains("edited"));
}

#[tokio::test(flavor = "multi_thread")]
async fn module_type_from_load_is_kept() {
  #[derive(Debug)]
  struct TextLoader;
  impl Plugin for TextLoader {
    fn name(&self) -> Cow<'static, str> {
      "text-loader".into()
    }
    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::Load
    }
    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
      if !args.id.ends_with(".txt") {
        return Ok(None);
      }
      let code = std::fs::read_to_string(args.id)?;
      Ok(Some(HookLoadOutput {
        code: code.into(),
        module_type: Some(ModuleType::Text),
        ..Default::default()
      }))
    }
    fn load_meta(&self) -> Option<PluginHookMeta> {
      Some(PluginHookMeta { order: None, cache: Some(true) })
    }
  }
  let project = Project::new(&[
    ("main.js", "import text from './a.txt';\nconsole.log(text);\n"),
    ("a.txt", "hello"),
  ]);
  let plugins: Vec<SharedPluginable> = vec![TextLoader::new_shared(TextLoader)];
  let first = build(project.options(Some(project.cache())), plugins.clone()).await;
  let second = build(project.options(Some(project.cache())), plugins).await;
  assert_eq!(second.files, first.files);
  assert!(second.files[0].1.contains("hello"));
}

#[tokio::test(flavor = "multi_thread")]
async fn assume_cacheable_hooks_does_not_turn_on_the_unsafe_inner_hooks() {
  let project = Project::new(&[
    ("main.js", "import './a.js';\nimport { b } from './b.js';\nconsole.log(b);\n"),
    ("a.js", "import x from 'data:text/javascript,export default 1';\nconsole.log(x);\n"),
    ("b.js", "export const b = 1;\n"),
  ]);
  let (plugin, calls) = Recorder::new("p", None);
  let cache = CacheOptions {
    assume_cacheable_hooks: Some(rolldown::AssumeCacheableHooks {
      resolve_id: Some(true),
      load: Some(true),
      transform: Some(true),
    }),
    ..project.cache()
  };
  let first = build(project.options(Some(cache.clone())), plugins(&[&plugin])).await;
  calls.take();
  // Before the data URL plugin set `cache: Some(false)`, this build failed to load the data URL
  // module.
  let second = build(project.options(Some(cache)), plugins(&[&plugin])).await;
  assert_eq!(
    calls.take(),
    ["load a.js", "resolveId data:text/javascript,export default 1", "transform a.js"]
  );
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_caught_emit_error_is_not_replayed() {
  #[derive(Debug)]
  struct BadEmit;
  impl Plugin for BadEmit {
    fn name(&self) -> Cow<'static, str> {
      "bad-emit".into()
    }
    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::Transform
    }
    async fn transform(
      &self,
      ctx: SharedTransformPluginContext,
      _args: &HookTransformArgs<'_>,
    ) -> HookTransformReturn {
      // An invalid name: the emit fails, and the plugin ignores the error.
      let _ = ctx.emit_file(
        EmittedAsset {
          name: Some("../outside.txt".into()),
          source: StrOrBytes::Str("x".into()),
          ..Default::default()
        },
        None,
        None,
      );
      Ok(None)
    }
    fn transform_meta(&self) -> Option<PluginHookMeta> {
      Some(PluginHookMeta { order: None, cache: Some(true) })
    }
  }
  let project = basic_project();
  let first =
    build(project.options(Some(project.cache())), vec![BadEmit::new_shared(BadEmit)]).await;
  let second =
    build(project.options(Some(project.cache())), vec![BadEmit::new_shared(BadEmit)]).await;
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_module_format_of_an_id_with_a_query_is_kept() {
  /// Loads an id with a query from the file without the query, as Vite does.
  #[derive(Debug)]
  struct QueryLoader;
  impl Plugin for QueryLoader {
    fn name(&self) -> Cow<'static, str> {
      "query-loader".into()
    }
    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::Load
    }
    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
      let Some((path, _)) = args.id.split_once('?') else { return Ok(None) };
      Ok(Some(HookLoadOutput { code: std::fs::read_to_string(path)?.into(), ..Default::default() }))
    }
    fn load_meta(&self) -> Option<PluginHookMeta> {
      Some(PluginHookMeta { order: None, cache: Some(true) })
    }
  }
  let project = Project::new(&[
    ("main.js", "import x from './b.mjs?v=1';\nconsole.log(x);\n"),
    ("b.mjs", "import c from './c.cjs';\nexport default c;\n"),
    ("c.cjs", "module.exports = { a: 1 };\n"),
  ]);
  let plugins = || vec![QueryLoader::new_shared(QueryLoader)];
  let clean = build(project.options(None), plugins()).await;
  build(project.options(Some(project.cache())), plugins()).await;
  // `main.js` is reused. The format of `./b.mjs?v=1` comes from its path without the query, as
  // the built-in resolver gives it.
  let second = build(project.options(Some(project.cache())), plugins()).await;
  assert_eq!(second.files, clean.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_build_dependency_that_changes_while_the_bundler_runs_turns_the_cache_off() {
  let project = basic_project();
  project.write("rolldown.config.js", "export default { banner: 'A' }");
  let (plugin, calls) = Recorder::new("p", Some(true));
  let cache =
    CacheOptions { build_dependencies: Some(vec!["rolldown.config.js".into()]), ..project.cache() };
  let logs = Arc::new(Mutex::new(vec![]));
  let options = with_log(project.options(Some(cache)), &logs);

  let mut bundler = Bundler::with_plugins(options.clone(), plugins(&[&plugin])).unwrap();
  bundler.generate().await.unwrap();
  calls.take();
  // The config changes, but this bundler still uses the options and plugins it was made with.
  project.write("rolldown.config.js", "export default { banner: 'B' }");
  bundler.generate().await.unwrap();
  assert_eq!(calls.take().len(), 8, "the cache is off for this bundler");
  bundler.close().await.unwrap();
  drop(bundler);

  // A restart with the new config finds nothing that the old config saved under its key.
  build(options, plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_on_log_does_not_fail_a_build_that_reuses_a_native_log() {
  let project = basic_project();
  let (mut plugin, calls) = Recorder::new("p", Some(true));
  Arc::get_mut(&mut plugin).unwrap().warn = vec!["b.js"];
  let failing_on_log = || {
    OnLog::new(Arc::new(|_level: LogLevel, _log| {
      Box::pin(async { Err(anyhow::anyhow!("onLog turned the warning into an error")) })
    }))
  };
  let options =
    BundlerOptions { on_log: Some(failing_on_log()), ..project.options(Some(project.cache())) };
  let mut bundler = Bundler::with_plugins(options.clone(), plugins(&[&plugin])).unwrap();
  bundler.generate().await.expect("a failing onLog of a native log does not fail the build");
  bundler.close().await.unwrap();
  calls.take();
  let mut bundler = Bundler::with_plugins(options, plugins(&[&plugin])).unwrap();
  bundler.generate().await.expect("the same holds when the log is replayed");
  bundler.close().await.unwrap();
  assert_eq!(calls.take(), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn watch_mode_is_an_input_of_key_1() {
  let project = basic_project();
  let (plugin, calls) = Recorder::new("p", Some(true));
  let in_watch_mode = CacheOptions { watch_mode: Some(true), ..project.cache() };
  build(project.options(Some(in_watch_mode.clone())), plugins(&[&plugin])).await;
  calls.take();
  build(project.options(Some(project.cache())), plugins(&[&plugin])).await;
  assert_eq!(calls.take().len(), 8, "results saved in watch mode are not reused outside it");
  build(project.options(Some(in_watch_mode)), plugins(&[&plugin])).await;
  assert_eq!(calls.take(), Vec::<String>::new());
}
