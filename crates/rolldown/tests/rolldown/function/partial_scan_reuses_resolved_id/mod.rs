use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use arcstr::ArcStr;
use rolldown::{BundleOutput, Bundler, BundlerOptions, ExperimentalOptions, InputItem};
use rolldown_common::{Output, ScanMode};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_std_utils::PathExt as _;

const VIRTUAL_ID: &str = "\0virtual";

/// Resolves only the specifier `virtual`, as most plugins with a virtual module do. A
/// `resolveId` call with the id of the module itself resolves to nothing.
#[derive(Debug)]
struct VirtualModule {
  entry_id: ArcStr,
  version: Arc<Mutex<u32>>,
  resolve_calls: Arc<Mutex<Vec<String>>>,
}

impl Plugin for VirtualModule {
  fn name(&self) -> Cow<'static, str> {
    "virtual-module".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    self.resolve_calls.lock().unwrap().push(args.specifier.to_string());
    if args.specifier == "virtual" {
      return Ok(Some(HookResolveIdOutput::from_id(ArcStr::from(VIRTUAL_ID))));
    }
    if args.specifier == self.entry_id {
      return Ok(Some(HookResolveIdOutput::from_id(self.entry_id.clone())));
    }
    Ok(None)
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let code = if args.id == self.entry_id {
      "import { value } from 'virtual'; console.log(value);".to_string()
    } else if args.id == VIRTUAL_ID {
      format!("export const value = 'VERSION_{}';", self.version.lock().unwrap())
    } else {
      return Ok(None);
    };
    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }
}

fn chunk_code(output: &BundleOutput) -> String {
  output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk.code.clone()),
      Output::Asset(_) => None,
    })
    .expect("the build should emit a chunk")
}

/// A partial scan loads a changed module again from the resolution that created it. It does not
/// call `resolveId` with the id of the module, which would not resolve a virtual module.
#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_reloads_a_virtual_module_without_resolve_id() {
  let cwd = std::env::current_dir().unwrap();
  let entry_id: ArcStr = cwd.join("entry.js").expect_to_str().into();
  let version = Arc::new(Mutex::new(1));
  let resolve_calls = Arc::new(Mutex::new(vec![]));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: None, import: entry_id.to_string() }]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(VirtualModule {
      entry_id: entry_id.clone(),
      version: Arc::clone(&version),
      resolve_calls: Arc::clone(&resolve_calls),
    })],
  )
  .unwrap();
  let output = bundler.incremental_generate(ScanMode::Full).await.unwrap();
  assert!(chunk_code(&output).contains("VERSION_1"));

  *version.lock().unwrap() = 2;
  resolve_calls.lock().unwrap().clear();
  let output =
    bundler.incremental_generate(ScanMode::Partial(vec![ArcStr::from(VIRTUAL_ID)])).await.unwrap();
  assert!(chunk_code(&output).contains("VERSION_2"));
  assert_eq!(*resolve_calls.lock().unwrap(), Vec::<String>::new());
}

/// A changed file that is not a module, such as a file that a plugin watches, is not loaded as
/// a new module.
#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_skips_a_changed_file_that_is_not_a_module() {
  let cwd = std::env::current_dir().unwrap();
  let entry_id: ArcStr = cwd.join("entry.js").expect_to_str().into();
  let resolve_calls = Arc::new(Mutex::new(vec![]));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: None, import: entry_id.to_string() }]),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(VirtualModule {
      entry_id: entry_id.clone(),
      version: Arc::new(Mutex::new(1)),
      resolve_calls: Arc::clone(&resolve_calls),
    })],
  )
  .unwrap();
  bundler.incremental_generate(ScanMode::Full).await.unwrap();

  resolve_calls.lock().unwrap().clear();
  let watched: ArcStr = cwd.join("watched.json").expect_to_str().into();
  let output = bundler.incremental_generate(ScanMode::Partial(vec![watched])).await.unwrap();
  assert!(chunk_code(&output).contains("VERSION_1"));
  assert_eq!(*resolve_calls.lock().unwrap(), Vec::<String>::new());
}
