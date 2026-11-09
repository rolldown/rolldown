use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use arcstr::ArcStr;
use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_common::{Output, ResolvedExternal, ScanMode};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_std_utils::PathExt as _;

/// `src/entry.js` imports `src/deep/dep.js`, which imports `lib/ext.js` as a path external once
/// `imports_external` is set.
#[derive(Debug)]
struct PathExternalPlugin {
  entry_id: ArcStr,
  dep_id: ArcStr,
  external_id: ArcStr,
  imports_external: Arc<AtomicBool>,
}

impl Plugin for PathExternalPlugin {
  fn name(&self) -> Cow<'static, str> {
    "path-external".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == "../../lib/ext.js" {
      return Ok(Some(HookResolveIdOutput {
        id: self.external_id.clone(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }
    let id = if args.specifier == "./deep/dep.js" || args.specifier == self.dep_id {
      &self.dep_id
    } else if args.specifier == self.entry_id {
      &self.entry_id
    } else {
      return Ok(None);
    };
    Ok(Some(HookResolveIdOutput::from_id(id.clone())))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = if args.id == self.entry_id {
      "import './deep/dep.js';"
    } else if args.id == self.dep_id {
      if self.imports_external.load(Ordering::Relaxed) {
        "import { value } from '../../lib/ext.js'; console.log(value);"
      } else {
        "console.log(0);"
      }
    } else {
      return Ok(None);
    };
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }
}

fn new_bundler(cwd: &std::path::Path, imports_external: bool) -> (Bundler, Arc<AtomicBool>) {
  let entry_id: ArcStr = cwd.join("src/entry.js").expect_to_str().into();
  let imports_external = Arc::new(AtomicBool::new(imports_external));
  let bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: entry_id.to_string(),
      }]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(PathExternalPlugin {
      entry_id,
      dep_id: cwd.join("src/deep/dep.js").expect_to_str().into(),
      external_id: cwd.join("lib/ext.js").expect_to_str().into(),
      imports_external: Arc::clone(&imports_external),
    })],
  )
  .unwrap();
  (bundler, imports_external)
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

#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_renders_new_path_external_as_full_build() {
  let cwd = std::env::current_dir().unwrap();
  let dep_id: ArcStr = cwd.join("src/deep/dep.js").expect_to_str().into();

  let (mut bundler, imports_external) = new_bundler(&cwd, false);
  bundler.incremental_generate(ScanMode::Full).await.unwrap();
  imports_external.store(true, Ordering::Relaxed);
  let rebuilt = tokio::time::timeout(
    Duration::from_secs(5),
    bundler.incremental_generate(ScanMode::Partial(vec![dep_id])),
  )
  .await
  .expect("the partial scan must not hang")
  .expect("the partial scan must succeed");

  let (mut full_bundler, _) = new_bundler(&cwd, true);
  let full = full_bundler.incremental_generate(ScanMode::Full).await.unwrap();

  // The import path is relative to `src/`, the entries' common directory, not to `src/deep/`.
  let rebuilt_code = chunk_code(&rebuilt.assets);
  assert!(rebuilt_code.contains("\"../lib/ext.js\""), "unexpected import path:\n{rebuilt_code}");
  assert_eq!(rebuilt_code, chunk_code(&full.assets));
}
