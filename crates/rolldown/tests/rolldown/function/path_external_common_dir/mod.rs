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

/// Loads `modules` (id, code, code once `rebuilt` is set), emits `emitted` as chunks in
/// `buildStart`, and resolves every import of `lib/ext.js` to that file as a path external.
#[derive(Debug)]
struct EmittedEntryPlugin {
  modules: Vec<(ArcStr, &'static str, &'static str)>,
  emitted: Vec<ArcStr>,
  external_id: ArcStr,
  rebuilt: Arc<AtomicBool>,
}

impl Plugin for EmittedEntryPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emitted-entry".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::ResolveId | HookUsage::Load
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    for id in &self.emitted {
      ctx.emit_chunk(rolldown_common::EmittedChunk { id: id.to_string(), ..Default::default() })?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier.ends_with("lib/ext.js") {
      return Ok(Some(HookResolveIdOutput {
        id: self.external_id.clone(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }
    let Some((id, _, _)) = self.modules.iter().find(|(id, _, _)| id == args.specifier) else {
      return Ok(None);
    };
    Ok(Some(HookResolveIdOutput::from_id(id.clone())))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let Some((_, code, rebuilt_code)) = self.modules.iter().find(|(id, _, _)| id == args.id) else {
      return Ok(None);
    };
    let code = if self.rebuilt.load(Ordering::Relaxed) { rebuilt_code } else { code };
    Ok(Some(HookLoadOutput { code: (*code).into(), ..Default::default() }))
  }
}

const IMPORTS_EXT: &str = "import { value } from '../lib/ext.js'; console.log(value);";

fn new_emitted_entry_bundler(
  input: &[ArcStr],
  modules: Vec<(ArcStr, &'static str, &'static str)>,
  emitted: Vec<ArcStr>,
) -> (Bundler, Arc<AtomicBool>) {
  let cwd = std::env::current_dir().unwrap();
  let rebuilt = Arc::new(AtomicBool::new(false));
  let bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(
        input.iter().map(|id| InputItem { name: None, import: id.to_string() }).collect(),
      ),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(EmittedEntryPlugin {
      modules,
      emitted,
      external_id: cwd.join("lib/ext.js").expect_to_str().into(),
      rebuilt: Arc::clone(&rebuilt),
    })],
  )
  .unwrap();
  (bundler, rebuilt)
}

fn chunk_codes(assets: &[Output]) -> Vec<String> {
  assets
    .iter()
    .filter_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_renders_path_external_without_input() {
  let cwd = std::env::current_dir().unwrap();
  let emitted_id: ArcStr = cwd.join("other/emitted.js").expect_to_str().into();
  let (mut bundler, _) = new_emitted_entry_bundler(
    &[],
    vec![(emitted_id.clone(), IMPORTS_EXT, IMPORTS_EXT)],
    vec![emitted_id],
  );
  let output =
    tokio::time::timeout(Duration::from_secs(5), bundler.incremental_generate(ScanMode::Full))
      .await
      .expect("the build must not hang")
      .expect("the build must succeed");

  // The import path is relative to `other/`, the directory of the emitted entry.
  let code = chunk_code(&output.assets);
  assert!(code.contains("\"../lib/ext.js\""), "unexpected import path:\n{code}");
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_renders_new_path_external_of_emitted_entry_as_full_build() {
  let cwd = std::env::current_dir().unwrap();
  let emitted_id: ArcStr = cwd.join("other/emitted.js").expect_to_str().into();
  let modules = vec![(emitted_id.clone(), "console.log(0);", IMPORTS_EXT)];

  let (mut bundler, rebuilt) =
    new_emitted_entry_bundler(&[], modules.clone(), vec![emitted_id.clone()]);
  bundler.incremental_generate(ScanMode::Full).await.unwrap();
  rebuilt.store(true, Ordering::Relaxed);
  let rebuilt_output = tokio::time::timeout(
    Duration::from_secs(5),
    bundler.incremental_generate(ScanMode::Partial(vec![emitted_id.clone()])),
  )
  .await
  .expect("the partial scan must not hang")
  .expect("the partial scan must succeed");

  let (mut full_bundler, full_rebuilt) = new_emitted_entry_bundler(&[], modules, vec![emitted_id]);
  full_rebuilt.store(true, Ordering::Relaxed);
  let full = full_bundler.incremental_generate(ScanMode::Full).await.unwrap();

  let rebuilt_code = chunk_code(&rebuilt_output.assets);
  assert!(rebuilt_code.contains("\"../lib/ext.js\""), "unexpected import path:\n{rebuilt_code}");
  assert_eq!(rebuilt_code, chunk_code(&full.assets));
}

#[tokio::test(flavor = "multi_thread")]
async fn emitted_entry_outside_input_moves_the_common_dir() {
  let cwd = std::env::current_dir().unwrap();
  let main_id: ArcStr = cwd.join("src/main.js").expect_to_str().into();
  let emitted_id: ArcStr = cwd.join("other/emitted.js").expect_to_str().into();
  let (mut bundler, _) = new_emitted_entry_bundler(
    std::slice::from_ref(&main_id),
    vec![
      (main_id.clone(), IMPORTS_EXT, IMPORTS_EXT),
      (emitted_id.clone(), IMPORTS_EXT, IMPORTS_EXT),
    ],
    vec![emitted_id],
  );
  let output = bundler.incremental_generate(ScanMode::Full).await.unwrap();

  // `src/` and `other/` share the current directory, so both chunks import `./lib/ext.js`.
  let codes = chunk_codes(&output.assets);
  assert_eq!(codes.len(), 2);
  for code in codes {
    assert!(code.contains("\"./lib/ext.js\""), "unexpected import path:\n{code}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn virtual_emitted_entry_renders_path_external_relative_to_cwd() {
  let virtual_id: ArcStr = "\0virtual-entry".into();
  let (mut bundler, _) = new_emitted_entry_bundler(
    &[],
    vec![(
      virtual_id.clone(),
      "import { value } from './lib/ext.js'; console.log(value);",
      "import { value } from './lib/ext.js'; console.log(value);",
    )],
    vec![virtual_id],
  );
  let output =
    tokio::time::timeout(Duration::from_secs(5), bundler.incremental_generate(ScanMode::Full))
      .await
      .expect("the build must not hang")
      .expect("the build must succeed");

  let code = chunk_code(&output.assets);
  assert!(code.contains("\"./lib/ext.js\""), "unexpected import path:\n{code}");
}
