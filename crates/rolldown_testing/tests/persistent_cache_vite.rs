//! The persistent scan cache with Rolldown's native Vite plugins. Each build uses a new `Bundler`
//! and new plugins, as a new process would, and reads the cache from disk.

use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicU32, Ordering},
  },
  time::Instant,
};

use rolldown::{
  BundleOutput, Bundler, BundlerOptions, CacheOptions, ExperimentalOptions, InputItem, LogLevel,
  OnLog,
  plugin::{
    __inner::SharedPluginable, HookBuildStartArgs, HookFilterArgs, HookLoadArgs, HookLoadReturn,
    HookNoopReturn, HookTransformArgs, HookTransformReturn, HookUsage, Plugin, PluginContext,
    PluginHookMeta, SharedLoadPluginContext, SharedTransformPluginContext,
  },
};
use rolldown_common::{
  DevModeOptions, ModuleInfo, ModuleType, NormalModule, ScanMode, WatcherChangeKind,
};
use rolldown_plugin_vite_alias::{Alias, ViteAliasPlugin};
use rolldown_plugin_vite_build_import_analysis::ViteBuildImportAnalysisPlugin;
use rolldown_plugin_vite_dynamic_import_vars::{ResolverFn, ViteDynamicImportVarsPlugin};
use rolldown_plugin_vite_import_glob::ViteImportGlobPlugin;
use rolldown_plugin_vite_json::ViteJsonPlugin;
use rolldown_plugin_vite_load_fallback::ViteLoadFallbackPlugin;
use rolldown_plugin_vite_module_preload_polyfill::ViteModulePreloadPolyfillPlugin;
use rolldown_plugin_vite_react_refresh_wrapper::{
  ViteReactRefreshWrapperPlugin, ViteReactRefreshWrapperPluginOptions,
};
use rolldown_plugin_vite_reporter::ViteReporterPlugin;
use rolldown_plugin_vite_resolve::{
  FinalizeOtherSpecifiersCallback, OnLogCallback, ResolveOptionsExternal, ResolveOptionsNoExternal,
  ViteResolveOptions, ViteResolvePlugin, ViteResolveResolveOptions,
};
use rolldown_plugin_vite_transform::ViteTransformPlugin;
use rolldown_utils::pattern_filter::StringOrRegex;
use rustc_hash::FxHashMap;

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
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
      .join(format!("rolldown-persistent-cache-vite-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    // The temp dir can be behind a symbolic link (macOS `/var` -> `/private/var`). Module ids
    // are real paths.
    let project = Self { dir: dunce::canonicalize(&dir).unwrap() };
    for (path, content) in files {
      let path = project.dir.join(path);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, content).unwrap();
    }
    project
  }

  fn root(&self) -> String {
    self.dir.to_string_lossy().into_owned()
  }

  fn options(&self) -> BundlerOptions {
    BundlerOptions {
      cwd: Some(self.dir.clone()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      experimental: Some(ExperimentalOptions {
        cache: Some(CacheOptions { dir: ".cache".into(), ..Default::default() }),
        ..Default::default()
      }),
      ..Default::default()
    }
  }
}

/// Records each `load` call on a file of the project. A module that reuses its saved result
/// does not call `load`.
#[derive(Debug, Default)]
struct Recorder {
  calls: Mutex<Vec<String>>,
}

impl Recorder {
  fn take(&self) -> Vec<String> {
    let mut calls = std::mem::take(&mut *self.calls.lock().unwrap());
    calls.sort();
    calls
  }
}

#[derive(Debug)]
struct RecorderPlugin(Arc<Recorder>);

impl Plugin for RecorderPlugin {
  fn name(&self) -> Cow<'static, str> {
    "recorder".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if !args.id.starts_with('\0') {
      let name = Path::new(args.id).file_name().unwrap().to_string_lossy().into_owned();
      self.0.calls.lock().unwrap().push(name);
    }
    Ok(None)
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: Some(true) })
  }
}

/// Passes the hooks to a reporter that the test can still read after the build.
#[derive(Debug)]
struct SharedReporter(Arc<ViteReporterPlugin>);

impl Plugin for SharedReporter {
  fn name(&self) -> Cow<'static, str> {
    self.0.name()
  }

  fn register_hook_usage(&self) -> HookUsage {
    self.0.register_hook_usage()
      & (HookUsage::ModuleParsed | HookUsage::Transform | HookUsage::BuildStart)
  }

  fn hook_filter_matches(&self, ctx: &PluginContext, args: HookFilterArgs<'_>) -> bool {
    self.0.hook_filter_matches(ctx, args)
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    self.0.build_start(ctx, args).await
  }

  async fn module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    normal_module: &NormalModule,
  ) -> HookNoopReturn {
    self.0.module_parsed(ctx, module_info, normal_module).await
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    self.0.transform(ctx, args).await
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    self.0.transform_meta()
  }
}

fn reporter(root: &str) -> Arc<ViteReporterPlugin> {
  Arc::new(ViteReporterPlugin {
    root: root.into(),
    assets_dir: "assets".into(),
    is_lib: false,
    is_tty: false,
    warn_large_chunks: false,
    chunk_limit: 500,
    report_compressed_size: false,
    chunk_count: AtomicU32::new(0),
    transformed_count: AtomicU32::new(0),
    latest_checkpoint: Arc::new(RwLock::new(Instant::now())),
    log_info: Some(Arc::new(|_| Box::pin(async { Ok(()) }))),
  })
}

fn resolve_plugin(
  root: &str,
  server: bool,
  on_warn: Option<Arc<OnLogCallback>>,
  finalize_other_specifiers: Option<Arc<FinalizeOtherSpecifiersCallback>>,
  on_debug: Option<Arc<OnLogCallback>>,
) -> ViteResolvePlugin {
  ViteResolvePlugin::new(ViteResolveOptions {
    resolve_options: ViteResolveResolveOptions {
      is_build: true,
      is_bundled: true,
      is_production: true,
      as_src: true,
      prefer_relative: false,
      is_require: None,
      root: root.into(),
      scan: false,
      main_fields: vec!["module".into(), "main".into()],
      conditions: vec!["import".into(), "default".into()],
      external_conditions: vec![],
      extensions: [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"].map(String::from).into(),
      try_index: true,
      try_prefix: None,
      preserve_symlinks: false,
      tsconfig_paths: false,
    },
    tsconfig: None,
    environment_consumer: if server { "server" } else { "client" }.into(),
    environment_name: if server { "ssr" } else { "client" }.into(),
    convert_external_require: false,
    builtins: vec![],
    external: ResolveOptionsExternal::Vec(vec![]),
    no_external: ResolveOptionsNoExternal::new_vec(vec![]),
    dedupe: vec![],
    disable_cache: false,
    legacy_inconsistent_cjs_interop: false,
    finalize_bare_specifier: None,
    finalize_other_specifiers,
    resolve_subpath_imports: Arc::new(|_, _, _, _| Box::pin(async { Ok(None) })),
    on_warn,
    on_debug,
    yarn_pnp: false,
  })
}

/// The native plugins in the order of a Vite build, with a recorder in front.
struct Setup {
  server: bool,
  on_warn: Option<Arc<OnLogCallback>>,
  dynamic_import_resolver: Option<Arc<ResolverFn>>,
  finalize_other_specifiers: Option<Arc<FinalizeOtherSpecifiersCallback>>,
  on_debug: Option<Arc<OnLogCallback>>,
  module_types: Option<FxHashMap<String, ModuleType>>,
}

impl Setup {
  fn client() -> Self {
    Self {
      server: false,
      on_warn: None,
      dynamic_import_resolver: None,
      finalize_other_specifiers: None,
      on_debug: None,
      module_types: None,
    }
  }

  fn plugins(
    &self,
    project: &Project,
    recorder: &Arc<Recorder>,
    reporter: &Arc<ViteReporterPlugin>,
  ) -> Vec<SharedPluginable> {
    let root = project.root();
    vec![
      RecorderPlugin::new_shared(RecorderPlugin(Arc::clone(recorder))),
      ViteAliasPlugin::new_shared(ViteAliasPlugin {
        entries: vec![Alias {
          find: StringOrRegex::String("@".into()),
          replacement: format!("{root}/src"),
        }],
      }),
      ViteModulePreloadPolyfillPlugin::new_shared(ViteModulePreloadPolyfillPlugin {
        is_server: self.server,
      }),
      ViteResolvePlugin::new_shared(resolve_plugin(
        &root,
        self.server,
        self.on_warn.clone(),
        self.finalize_other_specifiers.clone(),
        self.on_debug.clone(),
      )),
      ViteReactRefreshWrapperPlugin::new_shared(ViteReactRefreshWrapperPlugin::new(
        ViteReactRefreshWrapperPluginOptions {
          cwd: root.clone(),
          include: vec![],
          exclude: vec![],
          jsx_import_source: "react".into(),
          react_refresh_host: String::new(),
        },
      )),
      ViteTransformPlugin::new_shared(ViteTransformPlugin {
        root: project.dir.clone(),
        resolver: ViteTransformPlugin::new_resolver(false, None),
        ..Default::default()
      }),
      ViteJsonPlugin::new_shared(ViteJsonPlugin::default()),
      ViteDynamicImportVarsPlugin::new_shared(ViteDynamicImportVarsPlugin {
        resolver: self.dynamic_import_resolver.clone(),
        ..Default::default()
      }),
      ViteImportGlobPlugin::new_shared(ViteImportGlobPlugin {
        root: Some(root.clone()),
        ..Default::default()
      }),
      ViteBuildImportAnalysisPlugin::new_shared(ViteBuildImportAnalysisPlugin {
        preload_code: "export const __vitePreload = (f) => f();\n".into(),
        insert_preload: true,
        render_built_url: false,
        is_relative_base: false,
      }),
      SharedReporter::new_shared(SharedReporter(Arc::clone(reporter))),
      ViteLoadFallbackPlugin::new_shared(ViteLoadFallbackPlugin),
    ]
  }
}

struct BuildResult {
  files: Vec<(String, String)>,
  /// Logs without the cache's own logs.
  logs: Vec<String>,
  calls: Vec<String>,
  transformed_count: u32,
}

async fn build(project: &Project, setup: &Setup) -> BuildResult {
  let recorder = Arc::new(Recorder::default());
  let reporter = reporter(&project.root());
  let logs = Arc::new(Mutex::new(vec![]));
  let options = BundlerOptions {
    on_log: Some(OnLog::new(Arc::new({
      let logs = Arc::clone(&logs);
      move |level: LogLevel, log| {
        logs.lock().unwrap().push(format!("{level:?} {}", log.message));
        Box::pin(async { Ok(()) })
      }
    }))),
    module_types: setup.module_types.clone(),
    ..project.options()
  };
  let mut bundler =
    Bundler::with_plugins(options, setup.plugins(project, &recorder, &reporter)).unwrap();
  let BundleOutput { assets, .. } = bundler.generate().await.expect("build should succeed");
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
    logs,
    calls: recorder.take(),
    transformed_count: reporter.transformed_count.load(Ordering::SeqCst),
  }
}

fn vite_project() -> Project {
  Project::new(&[
    (
      "main.js",
      "import 'vite/modulepreload-polyfill';\n\
       import data from './data.json';\n\
       import { util } from '@/util';\n\
       console.log(data, util);\n\
       export const lazy = () => import('./lazy.js');\n",
    ),
    ("data.json", "{ \"a\": 1 }\n"),
    ("src/util.ts", "export const util: number = 1;\n"),
    ("lazy.js", "export const lazy = 'lazy';\n"),
    ("tsconfig.json", "{}\n"),
  ])
}

#[tokio::test(flavor = "multi_thread")]
async fn every_module_reuses_its_saved_result_with_the_vite_plugins() {
  let project = vite_project();
  let setup = Setup::client();

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["data.json", "lazy.js", "main.js", "util.ts"]);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, Vec::<String>::new(), "no hook of a saved module runs again");
  assert_eq!(second.files, first.files);
  assert_eq!(second.logs, first.logs);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_reporter_counts_the_same_modules_on_reuse() {
  let project = vite_project();
  let setup = Setup::client();

  let first = build(&project, &setup).await;
  // The four files, the two virtual modules and the runtime module.
  assert_eq!(first.transformed_count, 7);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, Vec::<String>::new());
  assert_eq!(second.transformed_count, first.transformed_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_reporter_does_not_count_a_module_that_skips_transform() {
  let project = Project::new(&[
    ("main.js", "import data from './data.bin';\nconsole.log(data);\n"),
    ("data.bin", "\u{0}\u{1}\u{2}"),
  ]);
  let mut setup = Setup::client();
  setup.module_types = Some(FxHashMap::from_iter([(".bin".to_string(), ModuleType::Binary)]));

  // Rolldown reads `data.bin` as bytes and runs no `transform` for it. The count holds
  // `main.js` and the runtime module.
  let first = build(&project, &setup).await;
  assert_eq!(first.transformed_count, 2);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, Vec::<String>::new());
  assert_eq!(second.transformed_count, first.transformed_count);
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_module_with_import_meta_glob_runs_again() {
  let project = Project::new(&[
    ("main.js", "import './glob.js';\nimport './plain.js';\n"),
    ("glob.js", "console.log(import.meta.glob('./pages/*.js', { eager: true }));\n"),
    ("plain.js", "console.log('plain');\n"),
    ("pages/a.js", "export const a = 'a';\n"),
  ]);
  let setup = Setup::client();

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["a.js", "glob.js", "main.js", "plain.js"]);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, ["glob.js"]);
  assert_eq!(second.files, first.files);
}

fn builtin_import_project() -> Project {
  Project::new(&[
    ("main.js", "import './uses-fs.js';\nimport './plain.js';\n"),
    ("uses-fs.js", "import fs from 'fs';\nconsole.log(fs);\n"),
    ("plain.js", "console.log('plain');\n"),
  ])
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resolve_warning_sent_to_on_warn_makes_the_importer_run_again() {
  let project = builtin_import_project();
  let warnings = Arc::new(Mutex::new(vec![]));
  let on_warn: Arc<OnLogCallback> = Arc::new({
    let warnings = Arc::clone(&warnings);
    move |message: String| {
      warnings.lock().unwrap().push(message);
      Box::pin(async { Ok(()) })
    }
  });
  let setup = Setup {
    server: true,
    on_warn: Some(on_warn),
    dynamic_import_resolver: None,
    finalize_other_specifiers: None,
    on_debug: None,
    ..Setup::client()
  };

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["main.js", "plain.js", "uses-fs.js"]);
  assert_eq!(warnings.lock().unwrap().len(), 1);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, ["uses-fs.js"]);
  assert_eq!(warnings.lock().unwrap().len(), 2, "the warning is sent again");
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resolve_warning_sent_to_ctx_warn_is_replayed() {
  let project = builtin_import_project();
  let setup = Setup {
    server: true,
    on_warn: None,
    dynamic_import_resolver: None,
    finalize_other_specifiers: None,
    on_debug: None,
    ..Setup::client()
  };

  let first = build(&project, &setup).await;
  assert!(
    first.logs.iter().any(|log| log.contains("Automatically externalized node built-in module")),
    "{:?}",
    first.logs
  );

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, Vec::<String>::new());
  assert_eq!(second.logs, first.logs);
  assert_eq!(second.files, first.files);
}

fn dynamic_import_project() -> Project {
  Project::new(&[
    ("main.js", "import './vars.js';\nimport './plain.js';\n"),
    ("vars.js", "export const load = (name) => import(`@/pages/${name}.js`);\n"),
    ("plain.js", "console.log('plain');\n"),
  ])
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dynamic_import_that_calls_the_resolver_makes_its_module_run_again() {
  let project = dynamic_import_project();
  let globs = Arc::new(Mutex::new(vec![]));
  let resolver: Arc<ResolverFn> = Arc::new({
    let globs = Arc::clone(&globs);
    move |glob: String, _importer: String| {
      globs.lock().unwrap().push(glob);
      Box::pin(async { Ok(None) })
    }
  });
  let setup = Setup {
    server: false,
    on_warn: None,
    dynamic_import_resolver: Some(resolver),
    finalize_other_specifiers: None,
    on_debug: None,
    ..Setup::client()
  };

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["main.js", "plain.js", "vars.js"]);
  assert_eq!(*globs.lock().unwrap(), ["@/pages/*.js"]);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, ["vars.js"]);
  assert_eq!(globs.lock().unwrap().len(), 2, "the resolver is called again");
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dynamic_import_without_a_resolver_is_reused() {
  let project = dynamic_import_project();
  let setup = Setup::client();

  let first = build(&project, &setup).await;
  let second = build(&project, &setup).await;
  assert_eq!(second.calls, Vec::<String>::new());
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resolve_that_calls_finalize_other_specifiers_makes_the_importer_run_again() {
  let project =
    Project::new(&[("main.js", "import './plain.js';\n"), ("plain.js", "console.log('plain');\n")]);
  // Vite passes this callback only to dev without bundling. Its result depends on the state of
  // Vite's dependency optimizer.
  let finalize: Arc<FinalizeOtherSpecifiersCallback> =
    Arc::new(|_resolved: &str, _raw: &str| Box::pin(async { Ok(None) }));
  let setup = Setup {
    server: false,
    on_warn: None,
    dynamic_import_resolver: None,
    finalize_other_specifiers: Some(finalize),
    on_debug: None,
    ..Setup::client()
  };

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["main.js", "plain.js"]);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, ["main.js"], "only the importer runs again");
  assert_eq!(second.files, first.files);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resolve_that_sends_a_debug_log_makes_the_importer_run_again() {
  let project =
    Project::new(&[("main.js", "import './plain.js';\n"), ("plain.js", "console.log('plain');\n")]);
  // Vite passes this callback only with `DEBUG=vite:resolve-details`.
  let logs = Arc::new(Mutex::new(vec![]));
  let on_debug: Arc<OnLogCallback> = Arc::new({
    let logs = Arc::clone(&logs);
    move |message: String| {
      logs.lock().unwrap().push(message);
      Box::pin(async { Ok(()) })
    }
  });
  let setup = Setup {
    server: false,
    on_warn: None,
    dynamic_import_resolver: None,
    finalize_other_specifiers: None,
    on_debug: Some(on_debug),
    ..Setup::client()
  };

  let first = build(&project, &setup).await;
  assert_eq!(first.calls, ["main.js", "plain.js"]);
  let first_logs = logs.lock().unwrap().len();
  assert!(first_logs > 0);

  let second = build(&project, &setup).await;
  assert_eq!(second.calls, ["main.js"], "only the importer runs again");
  assert_eq!(logs.lock().unwrap().len(), first_logs * 2, "the debug logs are sent again");
  assert_eq!(second.files, first.files);
}

/// Passes the hooks to an import-glob plugin that the test can still read.
#[derive(Debug)]
struct SharedGlob(Arc<ViteImportGlobPlugin>);

impl Plugin for SharedGlob {
  fn name(&self) -> Cow<'static, str> {
    self.0.name()
  }

  fn register_hook_usage(&self) -> HookUsage {
    self.0.register_hook_usage()
  }

  fn hook_filter_matches(&self, ctx: &PluginContext, args: HookFilterArgs<'_>) -> bool {
    self.0.hook_filter_matches(ctx, args)
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    args: &HookBuildStartArgs<'_>,
  ) -> HookNoopReturn {
    self.0.build_start(ctx, args).await
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    self.0.transform(ctx, args).await
  }

  fn transform_meta(&self) -> Option<PluginHookMeta> {
    self.0.transform_meta()
  }

  async fn module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    normal_module: &NormalModule,
  ) -> HookNoopReturn {
    self.0.module_parsed(ctx, module_info, normal_module).await
  }

  async fn hot_update(
    &self,
    ctx: &PluginContext,
    args: &rolldown::plugin::HookHotUpdateArgs,
  ) -> rolldown::plugin::HookHotUpdateReturn {
    self.0.hot_update(ctx, args).await
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dev_full_scan_that_reuses_a_saved_result_drops_the_old_globs() {
  const PLAIN: &str = "console.log('plain');\n";
  let project = Project::new(&[("main.js", PLAIN), ("pages/a.js", "export const a = 'a';\n")]);
  let main = project.dir.join("main.js");
  let main_id = main.to_string_lossy().into_owned();
  let glob = Arc::new(ViteImportGlobPlugin { root: Some(project.root()), ..Default::default() });
  let mut options = project.options();
  options.experimental.as_mut().unwrap().dev_mode = Some(DevModeOptions::default());
  let mut bundler =
    Bundler::with_plugins(options, vec![SharedGlob::new_shared(SharedGlob(Arc::clone(&glob)))])
      .unwrap();

  // `main.js` has no glob, so its result is saved.
  bundler.incremental_generate(ScanMode::Full).await.unwrap();
  assert!(glob.glob_matchers.is_empty());

  std::fs::write(&main, "console.log(import.meta.glob('./pages/*.js'));\n").unwrap();
  bundler.incremental_generate(ScanMode::Partial(vec![main_id.as_str().into()])).await.unwrap();
  assert!(glob.glob_matchers.contains_key(main_id.as_str()));

  // A full scan reuses the result of the first build, so `transform` does not run.
  std::fs::write(&main, PLAIN).unwrap();
  bundler.incremental_generate(ScanMode::Full).await.unwrap();
  assert!(glob.glob_matchers.is_empty(), "`main.js` has no glob any more");
  bundler.close().await.unwrap();

  // Creating a file in `pages/` does not update `main.js`.
  let ctx = PluginContext::new_napi_context();
  let added = project.dir.join("pages/b.js").to_string_lossy().into_owned();
  let update = glob
    .hot_update(
      &ctx,
      &rolldown::plugin::HookHotUpdateArgs {
        file: added.into(),
        kind: WatcherChangeKind::Create,
        modules: vec![],
      },
    )
    .await
    .unwrap();
  assert_eq!(update, None);
}
