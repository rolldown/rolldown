use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
};

use arcstr::ArcStr;
use rolldown::{
  BundleOutput, Bundler, BundlerOptions, ExperimentalOptions, InputItem, ResolveOptions,
};
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

/// An empty temporary directory for one test. Module ids are real paths, and the temporary
/// directory can be a symlink (macOS), so the path is canonical.
fn temp_dir(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("rolldown-{name}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  dunce::canonicalize(&dir).unwrap()
}

/// A `package.json` that cannot be parsed does not fail a partial scan. The changed module keeps
/// the data of the last read, and the scan fetches its new content. The next read after the fix
/// gives the new data.
#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_keeps_the_last_package_json_data_when_it_cannot_be_parsed() {
  let dir = temp_dir("partial-scan-package-json-unparsable");
  std::fs::create_dir_all(dir.join("pkg")).unwrap();
  let entry = dir.join("entry.js");
  let dep = dir.join("pkg/dep.js");
  let package_json = dir.join("pkg/package.json");
  std::fs::write(&entry, "import './pkg/dep.js'; globalThis.value = 'ENTRY';").unwrap();
  std::fs::write(&dep, "globalThis.value = 'DEP_1';").unwrap();
  std::fs::write(&package_json, r#"{"sideEffects":false}"#).unwrap();

  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem { name: None, import: entry.expect_to_str().to_string() }]),
    cwd: Some(dir.clone()),
    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  })
  .unwrap();
  let output = bundler.incremental_generate(ScanMode::Full).await.unwrap();
  assert!(!chunk_code(&output).contains("DEP_"));

  // `dep.js` keeps `sideEffects: false`, so it is still left out.
  std::fs::write(&package_json, "{ not json").unwrap();
  std::fs::write(&dep, "globalThis.value = 'DEP_2';").unwrap();
  bundler.clear_resolver_cache();
  let changed: ArcStr = dep.expect_to_str().into();
  let output = bundler.incremental_generate(ScanMode::Partial(vec![changed])).await.unwrap();
  assert!(!chunk_code(&output).contains("DEP_"));

  std::fs::write(&package_json, r#"{"sideEffects":true}"#).unwrap();
  std::fs::write(&dep, "globalThis.value = 'DEP_3';").unwrap();
  bundler.clear_resolver_cache();
  let changed: ArcStr = dep.expect_to_str().into();
  let output = bundler.incremental_generate(ScanMode::Partial(vec![changed])).await.unwrap();
  assert!(chunk_code(&output).contains("DEP_3"));

  std::fs::remove_dir_all(&dir).unwrap();
}

/// Writes a zip archive that stores `files` without compression, as Yarn PnP keeps a package.
/// The zip reader of Yarn PnP does not check the CRC, so it is zero.
fn write_zip(path: &Path, files: &[(&str, &str)]) {
  let mut data = Vec::new();
  let mut central_directory = Vec::new();
  for (name, content) in files {
    let offset = u32::try_from(data.len()).unwrap();
    let name_len = u16::try_from(name.len()).unwrap().to_le_bytes();
    let size = u32::try_from(content.len()).unwrap().to_le_bytes();
    // Local file header: signature, version, flags, method 0 (stored), time, date, CRC, sizes.
    data.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
    data.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    data.extend_from_slice(&size);
    data.extend_from_slice(&size);
    data.extend_from_slice(&name_len);
    data.extend_from_slice(&[0, 0]);
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(content.as_bytes());
    // Central directory header: the same fields, then attributes and the local header offset.
    central_directory.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
    central_directory.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    central_directory.extend_from_slice(&size);
    central_directory.extend_from_slice(&size);
    central_directory.extend_from_slice(&name_len);
    central_directory.extend_from_slice(&[0; 12]);
    central_directory.extend_from_slice(&offset.to_le_bytes());
    central_directory.extend_from_slice(name.as_bytes());
  }
  let central_directory_offset = u32::try_from(data.len()).unwrap();
  let count = u16::try_from(files.len()).unwrap().to_le_bytes();
  data.extend_from_slice(&central_directory);
  // End of central directory record.
  data.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
  data.extend_from_slice(&[0, 0, 0, 0]);
  data.extend_from_slice(&count);
  data.extend_from_slice(&count);
  data.extend_from_slice(&u32::try_from(central_directory.len()).unwrap().to_le_bytes());
  data.extend_from_slice(&central_directory_offset.to_le_bytes());
  data.extend_from_slice(&[0, 0]);
  std::fs::write(path, data).unwrap();
}

/// A partial scan reads the `package.json` of a module in a Yarn PnP zip archive again, so the
/// module keeps `sideEffects: false`.
#[tokio::test(flavor = "multi_thread")]
async fn partial_scan_reads_the_package_json_in_a_yarn_pnp_zip_archive() {
  let dir = temp_dir("partial-scan-yarn-pnp");
  write_zip(
    &dir.join("foo.zip"),
    &[
      ("node_modules/foo/package.json", r#"{"sideEffects":false}"#),
      ("node_modules/foo/index.js", "globalThis.value = 'FOO';"),
    ],
  );
  let foo = dir.join("foo.zip/node_modules/foo/index.js");
  let entry = dir.join("entry.js");
  std::fs::write(&entry, format!("import {:?}; globalThis.value = 'ENTRY';", foo.expect_to_str()))
    .unwrap();

  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem { name: None, import: entry.expect_to_str().to_string() }]),
    cwd: Some(dir.clone()),
    resolve: Some(ResolveOptions { yarn_pnp: Some(true), ..Default::default() }),
    experimental: Some(ExperimentalOptions { incremental_build: Some(true), ..Default::default() }),
    ..Default::default()
  })
  .unwrap();
  let output = bundler.incremental_generate(ScanMode::Full).await.unwrap();
  let code = chunk_code(&output);
  assert!(code.contains("ENTRY"));
  assert!(!code.contains("FOO"));

  let changed: ArcStr = foo.expect_to_str().into();
  let output = bundler.incremental_generate(ScanMode::Partial(vec![changed])).await.unwrap();
  assert!(!chunk_code(&output).contains("FOO"));

  std::fs::remove_dir_all(&dir).unwrap();
}
