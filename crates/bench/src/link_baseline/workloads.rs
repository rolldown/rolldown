use std::{
  ffi::OsStr,
  fmt::Write as _,
  fs,
  path::{Path, PathBuf},
};

use ignore::WalkBuilder;
use rolldown::{
  BundlerOptions, ChecksOptions, CodeSplittingMode, InputItem, OutputFormat, Platform,
  TreeshakeOptions, TsConfig,
};
use rolldown_fs::{FileSystem, MemoryFileSystem};
use rolldown_workspace::root_dir;
use serde::Serialize;
use xxhash_rust::xxh3::Xxh3;

pub const GENERATOR_VERSION: u32 = 2;
pub const DEFAULT_SEED: u64 = 0x6c69_6e6b_5f76_3031;

pub const SYNTHETIC_WORKLOAD_IDS: [&str; 8] = [
  "overhead-64",
  "wide-4096",
  "deep-1024",
  "scc-256x4",
  "export-star-1024",
  "cjs-2048",
  "json-2048",
  "dynamic-1024",
];
pub const REAL_WORKLOAD_IDS: [&str; 2] = ["three-r108", "rome"];

const THREE_COMMIT: &str = "7e0a78beb9317e580d7fa4da9b5b12be051c6feb";
const ROME_COMMIT: &str = "d95a3a7aab90773c9b36d9c82a08c8c4c6b68aa5";

#[derive(Clone, Copy)]
struct RealInputPin {
  file_count: usize,
  source_bytes: usize,
  input_digest: &'static str,
}

#[derive(Clone, Copy)]
struct RealWorkloadSpec {
  id: &'static str,
  commit: &'static str,
  entry: &'static str,
  included_paths: &'static [&'static str],
  input: RealInputPin,
  is_rome: bool,
}

const THREE_INPUT: RealInputPin = RealInputPin {
  file_count: 610,
  source_bytes: 1_474_106,
  input_digest: "d3c715c37ba5df677fe7e530088a4487",
};
const ROME_INPUT: RealInputPin = RealInputPin {
  file_count: 9_041,
  source_bytes: 15_108_932,
  input_digest: "771e707bc478f9316712dfb7647f4422",
};
const THREE_WORKLOAD: RealWorkloadSpec = RealWorkloadSpec {
  id: "three-r108",
  commit: THREE_COMMIT,
  entry: "./entry.js",
  included_paths: &["entry.js", "src"],
  input: THREE_INPUT,
  is_rome: false,
};
const ROME_WORKLOAD: RealWorkloadSpec = RealWorkloadSpec {
  id: "rome",
  commit: ROME_COMMIT,
  entry: "./src/entry.ts",
  included_paths: &["src"],
  input: ROME_INPUT,
  is_rome: true,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkloadManifest {
  pub id: String,
  pub generator_version: u32,
  pub seed: u64,
  pub source_module_count: Option<usize>,
  pub file_count: usize,
  pub source_bytes: usize,
  pub input_digest: String,
}

pub struct LinkBaselineWorkload {
  pub manifest: WorkloadManifest,
  pub options: BundlerOptions,
  pub fs: MemoryFileSystem,
}

struct WorkloadBuilder {
  id: &'static str,
  seed: u64,
  cwd: PathBuf,
  fs: MemoryFileSystem,
  hasher: Xxh3,
  file_count: usize,
  source_bytes: usize,
  circular_dependency_check: bool,
}

impl WorkloadBuilder {
  fn new(id: &'static str, seed: u64) -> Self {
    Self::new_with_circular_dependency_check(id, seed, false)
  }

  fn new_with_circular_dependency_check(
    id: &'static str,
    seed: u64,
    circular_dependency_check: bool,
  ) -> Self {
    let cwd = PathBuf::from(format!("/rolldown-link-baseline/{id}"));
    let mut hasher = Xxh3::default();
    hash_frame(&mut hasher, b"rolldown-link-baseline-input");
    hash_frame(&mut hasher, &GENERATOR_VERSION.to_le_bytes());
    hash_frame(&mut hasher, id.as_bytes());
    hash_frame(&mut hasher, &seed.to_le_bytes());
    hash_frame(&mut hasher, format!("format=esm;platform=browser;code-splitting=true;treeshake=true;minify=false;sourcemap=false;entry=entry.js;checks.circular-dependency={circular_dependency_check}").as_bytes());
    Self {
      id,
      seed,
      cwd,
      fs: MemoryFileSystem::default(),
      hasher,
      file_count: 0,
      source_bytes: 0,
      circular_dependency_check,
    }
  }

  fn add_file(&mut self, relative_path: &str, source: &str) {
    hash_frame(&mut self.hasher, relative_path.as_bytes());
    hash_frame(&mut self.hasher, source.as_bytes());
    self.fs.add_file(&self.cwd.join(relative_path), source);
    self.file_count += 1;
    self.source_bytes += source.len();
  }

  fn finish(self, source_module_count: usize) -> LinkBaselineWorkload {
    let options = BundlerOptions {
      cwd: Some(self.cwd),
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      platform: Some(Platform::Browser),
      format: Some(OutputFormat::Esm),
      treeshake: TreeshakeOptions::Boolean(true),
      code_splitting: Some(CodeSplittingMode::Bool(true)),
      checks: self
        .circular_dependency_check
        .then_some(ChecksOptions { circular_dependency: Some(true), ..Default::default() }),
      ..Default::default()
    };
    LinkBaselineWorkload {
      manifest: WorkloadManifest {
        id: self.id.to_string(),
        generator_version: GENERATOR_VERSION,
        seed: self.seed,
        source_module_count: Some(source_module_count),
        file_count: self.file_count,
        source_bytes: self.source_bytes,
        input_digest: format!("{:032x}", self.hasher.digest128()),
      },
      options,
      fs: self.fs,
    }
  }
}

fn hash_frame(hasher: &mut Xxh3, bytes: &[u8]) {
  hasher.update(&(bytes.len() as u64).to_le_bytes());
  hasher.update(bytes);
}

#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
  fn next(&mut self) -> u64 {
    self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = self.0;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
  }
}

pub fn synthetic_workload(id: &str) -> Result<LinkBaselineWorkload, String> {
  if id == "diagnostic-order" {
    return Ok(diagnostic_order_workload());
  }
  synthetic_workload_with_seed(id, DEFAULT_SEED)
}

pub fn baseline_workload(id: &str) -> Result<LinkBaselineWorkload, String> {
  match id {
    "three-r108" => real_workload(
      THREE_WORKLOAD,
      &root_dir().join("tmp/github/three"),
      &root_dir().join("tmp/bench/three"),
    ),
    "rome" => real_workload(
      ROME_WORKLOAD,
      &root_dir().join("tmp/github/rome"),
      &root_dir().join("tmp/bench/rome"),
    ),
    _ => synthetic_workload(id),
  }
}

pub fn synthetic_workload_with_seed(id: &str, seed: u64) -> Result<LinkBaselineWorkload, String> {
  match id {
    "overhead-64" => Ok(wide_workload("overhead-64", 64, seed)),
    "wide-4096" => Ok(wide_workload("wide-4096", 4096, seed)),
    "deep-1024" => Ok(deep_workload(seed)),
    "scc-256x4" => Ok(scc_workload(seed)),
    "export-star-1024" => Ok(export_star_workload(seed)),
    "cjs-2048" => Ok(cjs_workload(seed)),
    "json-2048" => Ok(json_workload(seed)),
    "dynamic-1024" => Ok(dynamic_workload(seed)),
    _ => {
      Err(format!("unknown workload `{id}`; expected one of {}", SYNTHETIC_WORKLOAD_IDS.join(", ")))
    }
  }
}

pub fn diagnostic_order_workload() -> LinkBaselineWorkload {
  let mut builder =
    WorkloadBuilder::new_with_circular_dependency_check("diagnostic-order", DEFAULT_SEED, true);
  builder.add_file(
    "entry.js",
    "import './cycle_a0.js';\n// The second cycle is discovered through the first cycle's dependency chain.\nimport { missing_a } from './missing_a.js';\nimport { missing_b } from './missing_b.js';\nconst required_a = require('./tla_a0.js');\nconst required_b = require('./tla_b0.js');\nglobalThis.__rolldownLinkBaseline = [missing_a, missing_b, required_a, required_b];\n",
  );
  builder.add_file("cycle_a0.js", "import './cycle_a1.js';\nglobalThis.__cycleA0 = true;\n");
  builder.add_file(
    "cycle_a1.js",
    "import './cycle_a0.js';\nimport './after_cycle_a.js';\nglobalThis.__cycleA1 = true;\n",
  );
  builder.add_file("after_cycle_a.js", "import './cycle_b0.js';\n");
  builder.add_file("cycle_b0.js", "import './cycle_b1.js';\nglobalThis.__cycleB0 = true;\n");
  builder.add_file("cycle_b1.js", "import './cycle_b0.js';\nglobalThis.__cycleB1 = true;\n");
  builder.add_file("tla_a0.js", "import { value_a } from './tla_a1.js';\nexport { value_a };\n");
  builder.add_file("tla_a1.js", "await Promise.resolve();\nexport const value_a = 'a';\n");
  builder.add_file("tla_b0.js", "import { value_b } from './tla_b1.js';\nexport { value_b };\n");
  builder.add_file("tla_b1.js", "await Promise.resolve();\nexport const value_b = 'b';\n");
  builder.add_file("missing_a.js", "export const present_a = 'a';\n");
  builder.add_file("missing_b.js", "export const present_b = 'b';\n");
  builder.finish(12)
}

fn real_workload(
  spec: RealWorkloadSpec,
  source_repository: &Path,
  transformed_directory: &Path,
) -> Result<LinkBaselineWorkload, String> {
  let RealWorkloadSpec { id, commit, entry, included_paths, input: expected_input, is_rome } = spec;
  let expected_commit = commit;
  verify_source_commit(id, source_repository, expected_commit)?;
  if !transformed_directory.join(entry.trim_start_matches("./")).is_file() {
    return Err(format!(
      "the transformed {id} input is missing at {}; run `vp exec node scripts/misc/setup-benchmark-input/index.js` after checking the pinned source commit",
      transformed_directory.display()
    ));
  }

  let mut files = WalkBuilder::new(transformed_directory)
    .hidden(false)
    .ignore(false)
    .git_ignore(false)
    .git_global(false)
    .git_exclude(false)
    .build()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_some_and(|file_type| file_type.is_file()))
    .filter_map(|entry| {
      let path = entry.into_path();
      let relative = path.strip_prefix(transformed_directory).ok()?;
      (!relative.components().any(|component| component.as_os_str() == OsStr::new(".git"))
        && included_paths.iter().any(|included| {
          relative == Path::new(included) || relative.starts_with(Path::new(included))
        }))
      .then_some((relative.to_path_buf(), path))
    })
    .collect::<Vec<_>>();
  files.sort_by(|(left, _), (right, _)| left.cmp(right));

  // Oxc's tsconfig discovery currently requires Rome's transformed absolute cwd.
  // Three has no tsconfig and uses a virtual cwd so its output is checkout-independent.
  let workload_cwd = if is_rome {
    transformed_directory.to_path_buf()
  } else {
    PathBuf::from(format!("/rolldown-link-baseline/{id}"))
  };
  let mut fs_image = MemoryFileSystem::default();
  let mut hasher = Xxh3::default();
  hash_frame(&mut hasher, b"rolldown-link-baseline-real-input");
  hash_frame(&mut hasher, &GENERATOR_VERSION.to_le_bytes());
  hash_frame(&mut hasher, id.as_bytes());
  hash_frame(&mut hasher, expected_commit.as_bytes());
  hash_frame(&mut hasher, &(included_paths.len() as u64).to_le_bytes());
  for included_path in included_paths {
    hash_frame(&mut hasher, included_path.as_bytes());
  }
  hash_frame(
    &mut hasher,
    if is_rome {
      b"format=esm;platform=browser;code-splitting=true;treeshake=true;minify=false;sourcemap=false;shim-missing-exports=true;tsconfig=src/tsconfig.json;entry=src/entry.ts"
    } else {
      b"format=esm;platform=browser;code-splitting=true;treeshake=true;minify=false;sourcemap=false;entry=entry.js"
    },
  );
  let mut source_bytes = 0;
  for (relative, absolute) in &files {
    let bytes = fs::read(absolute)
      .map_err(|error| format!("failed to read {}: {error}", absolute.display()))?;
    let relative_text = relative.to_string_lossy().replace('\\', "/");
    hash_frame(&mut hasher, relative_text.as_bytes());
    hash_frame(&mut hasher, &bytes);
    fs_image.add_file_bytes(&workload_cwd.join(relative), &bytes);
    source_bytes += bytes.len();
  }
  if is_rome && !fs_image.exists(&workload_cwd.join("src/tsconfig.json")) {
    return Err("the transformed Rome input did not preload src/tsconfig.json".to_string());
  }

  let options = BundlerOptions {
    cwd: Some(workload_cwd),
    input: Some(vec![InputItem { name: Some(id.to_string()), import: entry.to_string() }]),
    platform: Some(Platform::Browser),
    format: Some(OutputFormat::Esm),
    treeshake: TreeshakeOptions::Boolean(true),
    code_splitting: Some(CodeSplittingMode::Bool(true)),
    shim_missing_exports: is_rome.then_some(true),
    tsconfig: is_rome.then(|| TsConfig::Manual(PathBuf::from("src/tsconfig.json"))),
    ..Default::default()
  };
  let input_digest = format!("{:032x}", hasher.digest128());
  if files.len() != expected_input.file_count
    || source_bytes != expected_input.source_bytes
    || input_digest != expected_input.input_digest
  {
    return Err(format!(
      "transformed {id} input mismatch: expected {} files, {} bytes, digest {}; found {} files, {} bytes, digest {}",
      expected_input.file_count,
      expected_input.source_bytes,
      expected_input.input_digest,
      files.len(),
      source_bytes,
      input_digest
    ));
  }
  Ok(LinkBaselineWorkload {
    manifest: WorkloadManifest {
      id: id.to_string(),
      generator_version: GENERATOR_VERSION,
      seed: 0,
      source_module_count: None,
      file_count: files.len(),
      source_bytes,
      input_digest,
    },
    options,
    fs: fs_image,
  })
}

fn verify_source_commit(id: &str, repository: &Path, expected: &str) -> Result<(), String> {
  let actual = read_repository_head(repository).map_err(|error| {
    format!(
      "failed to inspect the pinned {id} source repository at {}: {error}",
      repository.display()
    )
  })?;
  if actual != expected {
    return Err(format!(
      "{id} source HEAD mismatch: expected {expected}, found {actual} at {}",
      repository.display()
    ));
  }
  Ok(())
}

pub fn read_repository_head(repository: &Path) -> Result<String, String> {
  let marker = repository.join(".git");
  let git_dir = if marker.is_dir() {
    marker
  } else {
    let marker_contents = fs::read_to_string(&marker)
      .map_err(|error| format!("failed to read {}: {error}", marker.display()))?;
    let path = marker_contents
      .trim()
      .strip_prefix("gitdir: ")
      .ok_or_else(|| format!("{} is not a Git directory marker", marker.display()))?;
    let path = PathBuf::from(path);
    if path.is_absolute() { path } else { repository.join(path) }
  };
  let head_path = git_dir.join("HEAD");
  let head = fs::read_to_string(&head_path)
    .map_err(|error| format!("failed to read {}: {error}", head_path.display()))?;
  let head = head.trim();
  if is_commit_hash(head) {
    return Ok(head.to_string());
  }
  let reference = head
    .strip_prefix("ref: ")
    .ok_or_else(|| format!("{} contains an invalid HEAD", head_path.display()))?;
  validate_reference_path(reference)?;

  let common_dir = read_common_dir(&git_dir)?;
  let loose_store = if is_worktree_private_reference(reference) { &git_dir } else { &common_dir };
  let loose_reference = loose_store.join(reference);
  match fs::read_to_string(&loose_reference) {
    Ok(value) => {
      let value = value.trim();
      if is_commit_hash(value) {
        return Ok(value.to_string());
      }
      return Err(format!("{} contains an invalid commit ID", loose_reference.display()));
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
    Err(error) => {
      return Err(format!("failed to read {}: {error}", loose_reference.display()));
    }
  }

  read_packed_reference(&common_dir.join("packed-refs"), reference)?.ok_or_else(|| {
    format!(
      "could not resolve {reference} in {} or {}",
      loose_store.display(),
      common_dir.join("packed-refs").display()
    )
  })
}

fn read_common_dir(git_dir: &Path) -> Result<PathBuf, String> {
  let marker = git_dir.join("commondir");
  match fs::read_to_string(&marker) {
    Ok(value) => {
      let value = value.trim();
      if value.is_empty() {
        return Err(format!("{} is empty", marker.display()));
      }
      let path = PathBuf::from(value);
      Ok(if path.is_absolute() { path } else { git_dir.join(path) })
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(git_dir.to_path_buf()),
    Err(error) => Err(format!("failed to read {}: {error}", marker.display())),
  }
}

fn validate_reference_path(reference: &str) -> Result<(), String> {
  let safe = reference.starts_with("refs/")
    && !reference.contains('\\')
    && !Path::new(reference).is_absolute()
    && reference.split('/').all(|component| !matches!(component, "" | "." | ".."));
  if safe { Ok(()) } else { Err(format!("HEAD contains an unsafe Git reference `{reference}`")) }
}

fn is_worktree_private_reference(reference: &str) -> bool {
  ["refs/bisect/", "refs/rewritten/", "refs/worktree/"]
    .iter()
    .any(|prefix| reference.starts_with(prefix))
}

fn read_packed_reference(path: &Path, reference: &str) -> Result<Option<String>, String> {
  let contents = match fs::read_to_string(path) {
    Ok(contents) => contents,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
  };

  for (index, line) in contents.lines().enumerate() {
    if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
      continue;
    }
    let mut fields = line.split(' ');
    let commit = fields.next().unwrap_or_default();
    let name = fields.next().unwrap_or_default();
    if commit.is_empty() || name.is_empty() || fields.next().is_some() || !is_commit_hash(commit) {
      return Err(format!("{} contains an invalid entry on line {}", path.display(), index + 1));
    }
    if name == reference {
      return Ok(Some(commit.to_string()));
    }
  }
  Ok(None)
}

fn is_commit_hash(value: &str) -> bool {
  value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn wide_workload(id: &'static str, module_count: usize, seed: u64) -> LinkBaselineWorkload {
  let mut builder = WorkloadBuilder::new(id, seed);
  let mut rng = SplitMix64(seed);
  let mut entry = String::new();
  for index in 1..module_count {
    let _ = writeln!(entry, "import {{ value_{index:04} }} from './module_{index:04}.js';");
  }
  entry.push_str("globalThis.__rolldownLinkBaseline = [");
  for index in 1..module_count {
    let _ = write!(entry, "value_{index:04},");
  }
  entry.push_str("];\n");
  builder.add_file("entry.js", &entry);

  for index in 1..module_count {
    let source =
      format!("export function value_{index:04}(input) {{ return input + {}; }}\n", rng.next());
    builder.add_file(&format!("module_{index:04}.js"), &source);
  }
  builder.finish(module_count)
}

fn deep_workload(seed: u64) -> LinkBaselineWorkload {
  const MODULE_COUNT: usize = 1024;
  let mut builder = WorkloadBuilder::new("deep-1024", seed);
  let mut rng = SplitMix64(seed);
  builder.add_file(
    "entry.js",
    "import { value_0001 } from './module_0001.js';\nglobalThis.__rolldownLinkBaseline = value_0001;\n",
  );
  for index in 1..MODULE_COUNT {
    let source = if index + 1 == MODULE_COUNT {
      format!("export function value_{index:04}(input) {{ return input + {}; }}\n", rng.next())
    } else {
      format!(
        "import {{ value_{next:04} }} from './module_{next:04}.js';\nexport function value_{index:04}(input) {{ return value_{next:04}(input) + {}; }}\n",
        rng.next(),
        next = index + 1
      )
    };
    builder.add_file(&format!("module_{index:04}.js"), &source);
  }
  builder.finish(MODULE_COUNT)
}

fn scc_workload(seed: u64) -> LinkBaselineWorkload {
  const SCC_COUNT: usize = 256;
  const SCC_SIZE: usize = 4;
  let mut builder = WorkloadBuilder::new("scc-256x4", seed);
  let mut rng = SplitMix64(seed);
  builder.add_file("entry.js", "import './scc_000_0.js';\n");

  for scc in 0..SCC_COUNT {
    for member in 0..SCC_SIZE {
      let next_member = (member + 1) % SCC_SIZE;
      let mut source = format!("import './scc_{scc:03}_{next_member}.js';\n");
      if member == 0 && scc + 1 < SCC_COUNT {
        let _ = writeln!(source, "import './scc_{next:03}_0.js';", next = scc + 1);
      }
      let _ = writeln!(
        source,
        "globalThis.__rolldownScc = (globalThis.__rolldownScc || 0) ^ {};",
        rng.next()
      );
      builder.add_file(&format!("scc_{scc:03}_{member}.js"), &source);
    }
  }
  builder.finish(1 + SCC_COUNT * SCC_SIZE)
}

fn export_star_workload(seed: u64) -> LinkBaselineWorkload {
  const LEAF_COUNT: usize = 1024;
  let mut builder = WorkloadBuilder::new("export-star-1024", seed);
  let mut rng = SplitMix64(seed);
  let mut current = Vec::with_capacity(LEAF_COUNT);

  for index in 0..LEAF_COUNT {
    let path = format!("leaf_{index:04}.js");
    builder.add_file(&path, &format!("export const value_{index:04} = {};\n", rng.next()));
    current.push(path);
  }

  let mut level = 0;
  while current.len() > 1 {
    let mut next = Vec::with_capacity(current.len() / 2);
    for (index, pair) in current.chunks_exact(2).enumerate() {
      let path = format!("star_{level:02}_{index:04}.js");
      builder.add_file(
        &path,
        &format!("export * from './{}';\nexport * from './{}';\n", pair[0], pair[1]),
      );
      next.push(path);
    }
    current = next;
    level += 1;
  }

  builder.add_file(
    "entry.js",
    &format!(
      "import * as namespace from './{}';\nglobalThis.__rolldownLinkBaseline = Object.keys(namespace).length;\n",
      current[0]
    ),
  );
  builder.finish(LEAF_COUNT * 2)
}

fn cjs_workload(seed: u64) -> LinkBaselineWorkload {
  const CJS_COUNT: usize = 2048;
  let mut builder = WorkloadBuilder::new("cjs-2048", seed);
  let mut rng = SplitMix64(seed);
  let mut entry = String::new();
  for index in 0..CJS_COUNT {
    let _ = writeln!(entry, "import cjs_{index:04} from './cjs_{index:04}.cjs';");
  }
  entry.push_str("globalThis.__rolldownLinkBaseline = [");
  for index in 0..CJS_COUNT {
    let _ = write!(entry, "cjs_{index:04}.value,");
  }
  entry.push_str("];\n");
  builder.add_file("entry.js", &entry);

  for index in 0..CJS_COUNT {
    builder.add_file(
      &format!("cjs_{index:04}.cjs"),
      &format!("module.exports = {{ value: {} }};\n", rng.next()),
    );
  }
  builder.finish(1 + CJS_COUNT)
}

fn json_workload(seed: u64) -> LinkBaselineWorkload {
  const JSON_COUNT: usize = 2048;
  let mut builder = WorkloadBuilder::new("json-2048", seed);
  let mut rng = SplitMix64(seed);
  let mut entry = String::new();
  for index in 0..JSON_COUNT {
    let _ = writeln!(entry, "import json_{index:04} from './data_{index:04}.json';");
  }
  entry.push_str("globalThis.__rolldownLinkBaseline = [");
  for index in 0..JSON_COUNT {
    let _ = write!(entry, "json_{index:04}.value,");
  }
  entry.push_str("];\n");
  builder.add_file("entry.js", &entry);

  for index in 0..JSON_COUNT {
    builder.add_file(
      &format!("data_{index:04}.json"),
      &format!("{{\"value\":{},\"label\":\"data_{index:04}\"}}\n", rng.next()),
    );
  }
  builder.finish(1 + JSON_COUNT)
}

fn dynamic_workload(seed: u64) -> LinkBaselineWorkload {
  const TARGET_COUNT: usize = 1024;
  let mut builder = WorkloadBuilder::new("dynamic-1024", seed);
  let mut rng = SplitMix64(seed);
  let mut entry = String::from("globalThis.__rolldownLinkBaseline = Promise.all([\n");
  for index in 0..TARGET_COUNT {
    let _ = writeln!(entry, "  import('./target_{index:04}.js'),");
  }
  entry.push_str("]).then((modules) => modules.length);\n");
  builder.add_file("entry.js", &entry);

  for index in 0..TARGET_COUNT {
    builder.add_file(
      &format!("target_{index:04}.js"),
      &format!("export const value_{index:04} = {};\n", rng.next()),
    );
  }
  builder.finish(1 + TARGET_COUNT)
}
