pub mod digest;
pub mod trace;
pub mod workloads;

use std::{
  fs,
  path::Path,
  process::Command,
  time::{Duration, Instant},
};

use crate::create_bench_context_with_memory_fs;
use rolldown::testing::{
  LinkBaselineSample, run_generate_with_link_baseline_observer, run_link_baseline_once,
};
use rolldown_workspace::root_dir;
use serde::Serialize;

use self::{
  digest::{DiagnosticDescriptor, DigestSet, describe_diagnostics, digest_capture, digest_failure},
  trace::{LinkTraceLayer, TraceSample},
  workloads::{LinkBaselineWorkload, WorkloadManifest},
};

pub const REPORT_SCHEMA_VERSION: u32 = 4;

const EXPECTED_RUSTC: &str = "rustc 1.97.0 (2d8144b78 2026-07-07)";
const EXPECTED_RUSTC_COMMIT: &str = "2d8144b7880597b6e6d3dfd63a9a9efae3f533d3";
const EXPECTED_RUSTC_HOST: &str = "x86_64-unknown-linux-gnu";
const EXPECTED_RUSTC_LLVM: &str = "22.1.6";
const EXPECTED_RUSTC_VERBOSE: &str = "rustc 1.97.0 (2d8144b78 2026-07-07)\nbinary: rustc\ncommit-hash: 2d8144b7880597b6e6d3dfd63a9a9efae3f533d3\ncommit-date: 2026-07-07\nhost: x86_64-unknown-linux-gnu\nrelease: 1.97.0\nLLVM version: 22.1.6";
const EXPECTED_CARGO: &str = "cargo 1.97.0 (c980f4866 2026-06-30)";
const EXPECTED_NODE: &str = "v24.12.0";
const EXPECTED_BUILD_COMMAND: &str =
  "cargo build --locked --release -p bench --features link-baseline --bin link-baseline";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BaselineMode {
  LinkTime,
  LinkTrace,
  Digest,
  LinkRss,
  ScanRss,
  BundleTime,
}

impl BaselineMode {
  pub fn parse(value: &str) -> Result<Self, String> {
    match value {
      "link-time" => Ok(Self::LinkTime),
      "link-trace" => Ok(Self::LinkTrace),
      "digest" => Ok(Self::Digest),
      "link-rss" => Ok(Self::LinkRss),
      "scan-rss" => Ok(Self::ScanRss),
      "bundle-time" => Ok(Self::BundleTime),
      _ => Err(format!(
        "unknown mode `{value}`; expected link-time, link-trace, digest, link-rss, scan-rss, or bundle-time"
      )),
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub struct BaselineConfig {
  pub mode: BaselineMode,
  pub warmups: usize,
  pub samples: usize,
  pub iterations_per_sample: usize,
  pub canonical: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimingStats {
  pub median_ns: u64,
  pub mad_ns: u64,
  pub relative_mad: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunEnvironment {
  pub canonical: bool,
  pub metadata_source: &'static str,
  pub git_commit: Option<String>,
  pub git_dirty: Option<bool>,
  pub rustc: Option<String>,
  pub rustc_verbose: Option<String>,
  pub cargo: Option<String>,
  pub node: Option<String>,
  pub os: String,
  pub architecture: String,
  pub cpu_model: Option<String>,
  pub cpu_governors: Option<Vec<String>>,
  pub load_average: Option<String>,
  pub locale: String,
  pub build_profile: String,
  pub allocator: String,
  pub rayon_num_threads: String,
  pub cpus_allowed_list: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildProvenance {
  pub verified: bool,
  pub version: String,
  pub git_commit: String,
  pub git_tree: String,
  pub git_dirty: Option<bool>,
  pub rustc: String,
  pub rustc_commit_hash: String,
  pub rustc_host: String,
  pub rustc_llvm: String,
  pub cargo: String,
  pub profile: String,
  pub opt_level: String,
  pub debug: String,
  pub debug_assertions: bool,
  pub lto: String,
  pub codegen_units: String,
  pub strip: String,
  pub target: String,
  pub host: String,
  pub rustflags_hex: String,
  pub command: String,
}

#[derive(Debug, Serialize)]
pub struct BaselineReport {
  pub schema_version: u32,
  pub manifest: WorkloadManifest,
  pub linked_module_count: Option<usize>,
  pub build: BuildProvenance,
  pub environment: RunEnvironment,
  pub mode: BaselineMode,
  pub warmups: usize,
  pub samples: usize,
  pub iterations_per_sample: usize,
  pub sample_ns: Vec<u64>,
  pub stats: Option<TimingStats>,
  pub trace_samples: Vec<TraceSample>,
  pub digests: Option<DigestSet>,
  pub pre_generate_diagnostics: Vec<DiagnosticDescriptor>,
  pub final_diagnostics: Vec<DiagnosticDescriptor>,
}

pub async fn run_baseline(
  config: BaselineConfig,
  workload: LinkBaselineWorkload,
) -> Result<BaselineReport, String> {
  run_baseline_inner(config, workload, None).await
}

pub async fn run_baseline_with_trace_layer(
  config: BaselineConfig,
  workload: LinkBaselineWorkload,
  trace_layer: &LinkTraceLayer,
) -> Result<BaselineReport, String> {
  run_baseline_inner(config, workload, Some(trace_layer)).await
}

async fn run_baseline_inner(
  config: BaselineConfig,
  workload: LinkBaselineWorkload,
  trace_layer: Option<&LinkTraceLayer>,
) -> Result<BaselineReport, String> {
  validate_config(config)?;
  let build = BuildProvenance::capture();
  if config.canonical {
    require_runner_environment(config.mode, true)?;
  }
  let environment = RunEnvironment::capture(config.mode, config.canonical, &build);
  if config.canonical {
    validate_captured_canonical_environment(config.mode, &environment)?;
  }
  let mut sample_ns = Vec::with_capacity(config.samples);
  let mut trace_samples = Vec::new();
  let mut digests = None;
  let mut pre_generate_diagnostics = Vec::new();
  let mut final_diagnostics = Vec::new();
  let mut linked_module_count = None;

  match config.mode {
    BaselineMode::Digest => {
      let mut context = create_context(&workload);
      let bundle =
        context.factory.create_bundle_with_fs(context.mem_fs.clone(), context.create_resolver());
      let observer_cwd = cwd(&workload).to_path_buf();
      let (generate_result, observation) =
        run_generate_with_link_baseline_observer(bundle, move |diagnostics, module_count| {
          (describe_diagnostics(diagnostics, &observer_cwd), module_count)
        })
        .await;
      let (diagnostics, module_count) = observation.ok_or_else(|| {
        "the pre-Generate link observer did not run; refusing to label an incomplete digest capture"
          .to_string()
      })?;
      pre_generate_diagnostics = diagnostics;
      record_linked_module_count(&mut linked_module_count, module_count)?;
      digests = Some(match generate_result {
        Ok(output) => {
          final_diagnostics = describe_diagnostics(&output.warnings, cwd(&workload));
          digest_capture(&output, &pre_generate_diagnostics, cwd(&workload))
        }
        Err(errors) => {
          final_diagnostics = describe_diagnostics(&errors, cwd(&workload));
          digest_failure(&errors, &pre_generate_diagnostics, cwd(&workload))
        }
      });
    }
    BaselineMode::LinkTrace => {
      let trace_layer = trace_layer.ok_or_else(|| {
        "link-trace mode requires the dedicated process-global trace collector".to_string()
      })?;
      for _ in 0..config.warmups {
        trace_layer.reset()?;
        let (_, trace) = run_traced_link(&workload, trace_layer).await?;
        validate_trace_attribution(&trace)?;
      }
      for _ in 0..config.samples {
        trace_layer.reset()?;
        let (sample, trace) = run_traced_link(&workload, trace_layer).await?;
        validate_trace_attribution(&trace)?;
        sample_ns.push(duration_ns(sample.elapsed));
        record_linked_module_count(&mut linked_module_count, sample.linked_module_count)?;
        trace_samples.push(trace);
      }
    }
    mode => {
      for _ in 0..config.warmups {
        let _ = run_timed_mode(mode, &workload).await?;
      }
      for _ in 0..config.samples {
        let mut elapsed = Duration::ZERO;
        for _ in 0..config.iterations_per_sample {
          let iteration = run_timed_mode(mode, &workload).await?;
          elapsed += iteration.elapsed;
          if let Some(module_count) = iteration.linked_module_count {
            record_linked_module_count(&mut linked_module_count, module_count)?;
          }
        }
        sample_ns.push(
          duration_ns(elapsed) / u64::try_from(config.iterations_per_sample).unwrap_or(u64::MAX),
        );
      }
    }
  }

  let stats = timing_stats(&sample_ns);
  Ok(BaselineReport {
    schema_version: REPORT_SCHEMA_VERSION,
    manifest: workload.manifest,
    linked_module_count,
    build,
    environment,
    mode: config.mode,
    warmups: config.warmups,
    samples: config.samples,
    iterations_per_sample: config.iterations_per_sample,
    sample_ns,
    stats,
    trace_samples,
    digests,
    pre_generate_diagnostics,
    final_diagnostics,
  })
}

async fn run_link(workload: &LinkBaselineWorkload) -> Result<LinkBaselineSample, String> {
  let mut context = create_context(workload);
  let bundle =
    context.factory.create_bundle_with_fs(context.mem_fs.clone(), context.create_resolver());
  run_link_baseline_once(bundle).await.map_err(|error| error.to_string())
}

async fn run_traced_link(
  workload: &LinkBaselineWorkload,
  layer: &LinkTraceLayer,
) -> Result<(LinkBaselineSample, TraceSample), String> {
  let sample = run_link(workload).await?;
  let trace = layer.sample(sample.elapsed)?;
  Ok((sample, trace))
}

fn validate_trace_attribution(trace: &TraceSample) -> Result<(), String> {
  if trace.detached_passes.is_empty() {
    return Ok(());
  }
  let passes = trace
    .detached_passes
    .iter()
    .map(|span| span.pass.as_deref().unwrap_or(span.name.as_str()))
    .collect::<Vec<_>>()
    .join(", ");
  Err(format!(
    "link trace recorded pass spans without LinkStage::link as their direct parent: {passes}"
  ))
}

async fn run_timed_mode(
  mode: BaselineMode,
  workload: &LinkBaselineWorkload,
) -> Result<TimedModeSample, String> {
  match mode {
    BaselineMode::LinkTime | BaselineMode::LinkRss => {
      let sample = run_link(workload).await?;
      Ok(TimedModeSample {
        elapsed: sample.elapsed,
        linked_module_count: Some(sample.linked_module_count),
      })
    }
    BaselineMode::ScanRss => {
      let mut context = create_context(workload);
      let bundle =
        context.factory.create_bundle_with_fs(context.mem_fs.clone(), context.create_resolver());
      let started = Instant::now();
      bundle.scan().await.map_err(|error| error.to_string())?;
      let elapsed = started.elapsed();
      Ok(TimedModeSample { elapsed, linked_module_count: None })
    }
    BaselineMode::BundleTime => {
      let mut context = create_context(workload);
      let bundle =
        context.factory.create_bundle_with_fs(context.mem_fs.clone(), context.create_resolver());
      let started = Instant::now();
      let output = bundle.generate().await.map_err(|error| error.to_string())?;
      let elapsed = started.elapsed();
      std::hint::black_box(&output);
      Ok(TimedModeSample { elapsed, linked_module_count: None })
    }
    BaselineMode::LinkTrace | BaselineMode::Digest => {
      Err("the selected mode needs its dedicated execution path".to_string())
    }
  }
}

struct TimedModeSample {
  elapsed: Duration,
  linked_module_count: Option<usize>,
}

fn record_linked_module_count(slot: &mut Option<usize>, actual: usize) -> Result<(), String> {
  match slot {
    Some(expected) if *expected != actual => Err(format!(
      "linked module count changed between samples: expected {expected}, found {actual}"
    )),
    Some(_) => Ok(()),
    None => {
      *slot = Some(actual);
      Ok(())
    }
  }
}

fn create_context(workload: &LinkBaselineWorkload) -> crate::BenchContext {
  create_bench_context_with_memory_fs(&workload.options, workload.fs.clone())
}

fn cwd(workload: &LinkBaselineWorkload) -> &Path {
  workload.options.cwd.as_deref().expect("synthetic workloads always set cwd")
}

fn validate_config(config: BaselineConfig) -> Result<(), String> {
  if config.samples == 0 {
    return Err("samples must be greater than zero".to_string());
  }
  if config.iterations_per_sample == 0 {
    return Err("iterations per sample must be greater than zero".to_string());
  }
  if matches!(config.mode, BaselineMode::Digest) && (config.warmups != 0 || config.samples != 1) {
    return Err("digest mode requires exactly --warmups 0 --samples 1".to_string());
  }
  if matches!(config.mode, BaselineMode::LinkRss | BaselineMode::ScanRss)
    && (config.warmups != 0 || config.samples != 1)
  {
    return Err("RSS modes require exactly --warmups 0 --samples 1".to_string());
  }
  let single_iteration_mode = match config.mode {
    BaselineMode::Digest => Some("digest"),
    BaselineMode::LinkTrace => Some("link-trace"),
    BaselineMode::LinkRss | BaselineMode::ScanRss => Some("RSS"),
    BaselineMode::LinkTime | BaselineMode::BundleTime => None,
  };
  if let Some(mode) = single_iteration_mode
    && config.iterations_per_sample != 1
  {
    return Err(format!("{mode} mode requires exactly --iterations-per-sample 1"));
  }
  Ok(())
}

pub fn timing_stats(samples: &[u64]) -> Option<TimingStats> {
  if samples.is_empty() {
    return None;
  }
  let mut values = samples.to_vec();
  values.sort_unstable();
  let median_ns = median(&values);
  let mut deviations = values.iter().map(|value| value.abs_diff(median_ns)).collect::<Vec<_>>();
  deviations.sort_unstable();
  let mad_ns = median(&deviations);
  let relative_mad = if median_ns == 0 { 0.0 } else { mad_ns as f64 / median_ns as f64 };
  Some(TimingStats { median_ns, mad_ns, relative_mad })
}

fn median(sorted: &[u64]) -> u64 {
  let middle = sorted.len() / 2;
  if sorted.len() % 2 == 1 {
    sorted[middle]
  } else {
    let left = sorted[middle - 1];
    let right = sorted[middle];
    left / 2 + right / 2 + (left % 2 + right % 2) / 2
  }
}

fn duration_ns(duration: Duration) -> u64 {
  u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

impl RunEnvironment {
  fn capture(mode: BaselineMode, canonical: bool, build: &BuildProvenance) -> Self {
    let rss_mode = matches!(mode, BaselineMode::LinkRss | BaselineMode::ScanRss);
    Self {
      canonical,
      metadata_source: if rss_mode { "environment" } else { "subprocess" },
      git_commit: metadata_value(
        rss_mode,
        "ROLLDOWN_LINK_BASELINE_GIT_COMMIT",
        "git",
        &["rev-parse", "HEAD"],
      ),
      git_dirty: if rss_mode {
        std::env::var("ROLLDOWN_LINK_BASELINE_GIT_DIRTY").ok().and_then(|value| parse_bool(&value))
      } else {
        command_output("git", &["status", "--porcelain=v1", "--untracked-files=normal"])
          .map(|status| !status.is_empty())
      },
      rustc: metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_RUSTC", "rustc", &["--version"]),
      rustc_verbose: metadata_value(
        rss_mode,
        "ROLLDOWN_LINK_BASELINE_RUSTC_VERBOSE",
        "rustc",
        &["-vV"],
      ),
      cargo: metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_CARGO", "cargo", &["--version"]),
      node: metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_NODE", "node", &["--version"]),
      os: std::env::consts::OS.to_string(),
      architecture: std::env::consts::ARCH.to_string(),
      cpu_model: read_cpu_model(),
      cpu_governors: read_cpu_governors(),
      load_average: read_load_average(),
      locale: std::env::var("LC_ALL").unwrap_or_else(|_| "<unset>".to_string()),
      build_profile: build.profile.clone(),
      allocator: "mimalloc".to_string(),
      rayon_num_threads: std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "<unset>".into()),
      cpus_allowed_list: read_cpus_allowed_list(),
    }
  }
}

impl BuildProvenance {
  fn capture() -> Self {
    let version = env!("ROLLDOWN_LINK_BASELINE_BUILD_PROVENANCE_VERSION").to_string();
    Self {
      verified: version == "1",
      version,
      git_commit: env!("ROLLDOWN_LINK_BASELINE_BUILD_GIT_COMMIT").to_string(),
      git_tree: env!("ROLLDOWN_LINK_BASELINE_BUILD_GIT_TREE").to_string(),
      git_dirty: parse_bool(env!("ROLLDOWN_LINK_BASELINE_BUILD_GIT_DIRTY")),
      rustc: env!("ROLLDOWN_LINK_BASELINE_BUILD_RUSTC").to_string(),
      rustc_commit_hash: env!("ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_COMMIT_HASH").to_string(),
      rustc_host: env!("ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_HOST").to_string(),
      rustc_llvm: env!("ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_LLVM").to_string(),
      cargo: env!("ROLLDOWN_LINK_BASELINE_BUILD_CARGO").to_string(),
      profile: env!("ROLLDOWN_LINK_BASELINE_BUILD_PROFILE").to_string(),
      opt_level: env!("ROLLDOWN_LINK_BASELINE_BUILD_OPT_LEVEL").to_string(),
      debug: env!("ROLLDOWN_LINK_BASELINE_BUILD_DEBUG").to_string(),
      debug_assertions: cfg!(debug_assertions),
      lto: env!("ROLLDOWN_LINK_BASELINE_BUILD_LTO").to_string(),
      codegen_units: env!("ROLLDOWN_LINK_BASELINE_BUILD_CODEGEN_UNITS").to_string(),
      strip: env!("ROLLDOWN_LINK_BASELINE_BUILD_STRIP").to_string(),
      target: env!("ROLLDOWN_LINK_BASELINE_BUILD_TARGET").to_string(),
      host: env!("ROLLDOWN_LINK_BASELINE_BUILD_HOST").to_string(),
      rustflags_hex: env!("ROLLDOWN_LINK_BASELINE_BUILD_RUSTFLAGS_HEX").to_string(),
      command: env!("ROLLDOWN_LINK_BASELINE_BUILD_COMMAND").to_string(),
    }
  }
}

pub fn require_runner_environment(mode: BaselineMode, canonical: bool) -> Result<(), String> {
  let value = std::env::var("RAYON_NUM_THREADS")
    .map_err(|_| "RAYON_NUM_THREADS must be set before starting the runner".to_string())?;
  let threads = value
    .parse::<usize>()
    .map_err(|_| format!("RAYON_NUM_THREADS must be a positive integer, got `{value}`"))?;
  if threads == 0 {
    return Err("RAYON_NUM_THREADS must be greater than zero".to_string());
  }
  let dirty = if matches!(mode, BaselineMode::LinkRss | BaselineMode::ScanRss) {
    for variable in [
      "ROLLDOWN_LINK_BASELINE_GIT_COMMIT",
      "ROLLDOWN_LINK_BASELINE_GIT_DIRTY",
      "ROLLDOWN_LINK_BASELINE_RUSTC",
      "ROLLDOWN_LINK_BASELINE_RUSTC_VERBOSE",
      "ROLLDOWN_LINK_BASELINE_CARGO",
      "ROLLDOWN_LINK_BASELINE_NODE",
    ] {
      required_environment_value(variable)?;
    }
    let value = required_environment_value("ROLLDOWN_LINK_BASELINE_GIT_DIRTY")?;
    parse_bool(&value).ok_or_else(|| {
      format!("ROLLDOWN_LINK_BASELINE_GIT_DIRTY must be true or false, got `{value}`")
    })?
  } else {
    !command_output("git", &["status", "--porcelain=v1", "--untracked-files=normal"])
      .ok_or_else(|| "failed to inspect the repository worktree".to_string())?
      .is_empty()
  };
  if dirty && canonical {
    return Err(
      "the repository worktree is dirty; commit the runner before a canonical baseline or pass --development for an explicitly non-canonical run"
        .to_string(),
    );
  }
  if !canonical {
    return Ok(());
  }

  validate_build_provenance(mode, &BuildProvenance::capture())?;
  if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
    return Err(format!(
      "canonical baselines require Linux x86_64, found {} {}",
      std::env::consts::OS,
      std::env::consts::ARCH
    ));
  }
  let expected_threads = if mode == BaselineMode::Digest { &[1, 4][..] } else { &[4][..] };
  if !expected_threads.contains(&threads) {
    return Err(format!(
      "canonical {mode:?} baselines require RAYON_NUM_THREADS={}, found {threads}",
      expected_threads.iter().map(ToString::to_string).collect::<Vec<_>>().join(" or ")
    ));
  }
  require_exact("LC_ALL", &std::env::var("LC_ALL").unwrap_or_default(), "C")?;
  require_exact(
    "CPU model",
    &read_cpu_model().unwrap_or_default(),
    "13th Gen Intel(R) Core(TM) i5-13500H",
  )?;
  require_exact("CPU affinity", &read_cpus_allowed_list().unwrap_or_default(), "0,2,4,6")?;
  let governors = read_cpu_governors()
    .ok_or_else(|| "failed to inspect CPU governors for canonical CPUs 0,2,4,6".to_string())?;
  let expected_governors = [
    "0:performance".to_string(),
    "2:performance".to_string(),
    "4:performance".to_string(),
    "6:performance".to_string(),
  ];
  if governors != expected_governors {
    return Err(format!(
      "canonical baselines require performance governors on CPUs 0,2,4,6; found {}",
      governors.join(", ")
    ));
  }

  let rss_mode = matches!(mode, BaselineMode::LinkRss | BaselineMode::ScanRss);
  let current_head = workloads::read_repository_head(&root_dir())?;
  let reported_head = required_metadata_value(
    rss_mode,
    "ROLLDOWN_LINK_BASELINE_GIT_COMMIT",
    "git",
    &["rev-parse", "HEAD"],
  )?;
  require_exact("Git commit", &reported_head, &current_head)?;
  if dirty {
    return Err("canonical baselines require git_dirty=false".to_string());
  }
  require_exact(
    "rustc --version",
    &required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_RUSTC", "rustc", &["--version"])?,
    EXPECTED_RUSTC,
  )?;
  require_exact(
    "rustc -vV",
    &required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_RUSTC_VERBOSE", "rustc", &["-vV"])?,
    EXPECTED_RUSTC_VERBOSE,
  )?;
  require_exact(
    "cargo --version",
    &required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_CARGO", "cargo", &["--version"])?,
    EXPECTED_CARGO,
  )?;
  require_exact(
    "node --version",
    &required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_NODE", "node", &["--version"])?,
    EXPECTED_NODE,
  )?;
  Ok(())
}

fn validate_build_provenance(mode: BaselineMode, build: &BuildProvenance) -> Result<(), String> {
  let rss_mode = matches!(mode, BaselineMode::LinkRss | BaselineMode::ScanRss);
  let runtime_rustc =
    required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_RUSTC", "rustc", &["--version"])?;
  let runtime_cargo =
    required_metadata_value(rss_mode, "ROLLDOWN_LINK_BASELINE_CARGO", "cargo", &["--version"])?;
  validate_build_provenance_values(
    build,
    &workloads::read_repository_head(&root_dir())?,
    &runtime_rustc,
    &runtime_cargo,
  )
}

#[doc(hidden)]
pub fn validate_build_provenance_values(
  build: &BuildProvenance,
  current_head: &str,
  runtime_rustc: &str,
  runtime_cargo: &str,
) -> Result<(), String> {
  if !build.verified {
    return Err(
      "canonical baselines require a runner built by `just build-link-baseline`; this binary has no verified build provenance"
        .to_string(),
    );
  }
  require_exact("build provenance version", &build.version, "1")?;
  require_exact("build Git commit", &build.git_commit, current_head)?;
  if build.git_dirty != Some(false) {
    return Err("canonical baselines require a binary built from a clean worktree".to_string());
  }
  if build.git_tree.len() != 40 || !build.git_tree.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return Err(format!("canonical baseline build Git tree is invalid: `{}`", build.git_tree));
  }

  require_exact("build rustc", &build.rustc, runtime_rustc)?;
  require_exact("build rustc", &build.rustc, EXPECTED_RUSTC)?;
  require_exact("build rustc commit", &build.rustc_commit_hash, EXPECTED_RUSTC_COMMIT)?;
  require_exact("build rustc host", &build.rustc_host, EXPECTED_RUSTC_HOST)?;
  require_exact("build LLVM", &build.rustc_llvm, EXPECTED_RUSTC_LLVM)?;
  require_exact("build Cargo", &build.cargo, runtime_cargo)?;
  require_exact("build Cargo", &build.cargo, EXPECTED_CARGO)?;
  require_exact("build profile", &build.profile, "release")?;
  require_exact("build opt-level", &build.opt_level, "3")?;
  require_exact("build debug setting", &build.debug, "false")?;
  if build.debug_assertions {
    return Err("canonical baselines require debug assertions to be disabled".to_string());
  }
  require_exact("build LTO", &build.lto, "fat")?;
  require_exact("build codegen units", &build.codegen_units, "1")?;
  require_exact("build strip setting", &build.strip, "symbols")?;
  require_exact("build target", &build.target, EXPECTED_RUSTC_HOST)?;
  require_exact("build host", &build.host, EXPECTED_RUSTC_HOST)?;
  require_exact("build rustflags", &build.rustflags_hex, "")?;
  require_exact("build command", &build.command, EXPECTED_BUILD_COMMAND)
}

fn validate_captured_canonical_environment(
  mode: BaselineMode,
  environment: &RunEnvironment,
) -> Result<(), String> {
  if !environment.canonical {
    return Err("a canonical report must be marked canonical".to_string());
  }
  if environment.git_dirty != Some(false) {
    return Err("a canonical report requires git_dirty=false".to_string());
  }
  require_exact(
    "metadata source",
    environment.metadata_source,
    if matches!(mode, BaselineMode::LinkRss | BaselineMode::ScanRss) {
      "environment"
    } else {
      "subprocess"
    },
  )?;
  require_exact("build profile", &environment.build_profile, "release")?;
  require_exact("OS", &environment.os, "linux")?;
  require_exact("architecture", &environment.architecture, "x86_64")?;
  require_exact("LC_ALL", &environment.locale, "C")?;
  require_exact(
    "CPU model",
    environment.cpu_model.as_deref().unwrap_or_default(),
    "13th Gen Intel(R) Core(TM) i5-13500H",
  )?;
  require_exact(
    "CPU affinity",
    environment.cpus_allowed_list.as_deref().unwrap_or_default(),
    "0,2,4,6",
  )?;
  let expected_governors = ["0:performance", "2:performance", "4:performance", "6:performance"];
  let governors = environment
    .cpu_governors
    .as_deref()
    .ok_or_else(|| "a canonical report must contain CPU governors".to_string())?;
  if !governors.iter().map(String::as_str).eq(expected_governors) {
    return Err(format!(
      "canonical baseline CPU governor mismatch: expected `{}`, found `{}`",
      expected_governors.join(", "),
      governors.join(", ")
    ));
  }
  let threads = environment.rayon_num_threads.parse::<usize>().map_err(|_| {
    format!("invalid captured RAYON_NUM_THREADS `{}`", environment.rayon_num_threads)
  })?;
  let expected_threads = if mode == BaselineMode::Digest { &[1, 4][..] } else { &[4][..] };
  if !expected_threads.contains(&threads) {
    return Err(format!(
      "canonical captured RAYON_NUM_THREADS must be {}, found {threads}",
      expected_threads.iter().map(ToString::to_string).collect::<Vec<_>>().join(" or ")
    ));
  }
  require_exact(
    "Git commit",
    environment.git_commit.as_deref().unwrap_or_default(),
    &workloads::read_repository_head(&root_dir())?,
  )?;
  require_exact(
    "rustc --version",
    environment.rustc.as_deref().unwrap_or_default(),
    EXPECTED_RUSTC,
  )?;
  require_exact(
    "rustc -vV",
    environment.rustc_verbose.as_deref().unwrap_or_default(),
    EXPECTED_RUSTC_VERBOSE,
  )?;
  require_exact(
    "cargo --version",
    environment.cargo.as_deref().unwrap_or_default(),
    EXPECTED_CARGO,
  )?;
  require_exact("node --version", environment.node.as_deref().unwrap_or_default(), EXPECTED_NODE)?;
  require_exact("allocator", &environment.allocator, "mimalloc")
}

fn require_exact(label: &str, actual: &str, expected: &str) -> Result<(), String> {
  if actual == expected {
    Ok(())
  } else {
    Err(format!("canonical baseline {label} mismatch: expected `{expected}`, found `{actual}`"))
  }
}

fn required_environment_value(variable: &str) -> Result<String, String> {
  let value = std::env::var(variable)
    .map_err(|_| format!("{variable} must be captured outside the timed process for RSS modes"))?;
  let value = value.trim();
  if value.is_empty() {
    return Err(format!("{variable} must not be empty"));
  }
  Ok(value.to_string())
}

fn required_metadata_value(
  environment_only: bool,
  variable: &str,
  program: &str,
  args: &[&str],
) -> Result<String, String> {
  if environment_only {
    required_environment_value(variable)
  } else {
    command_output(program, args)
      .filter(|value| !value.is_empty())
      .ok_or_else(|| format!("failed to run {program} {}", args.join(" ")))
  }
}

fn metadata_value(
  environment_only: bool,
  variable: &str,
  program: &str,
  args: &[&str],
) -> Option<String> {
  if environment_only {
    std::env::var(variable)
      .ok()
      .map(|value| value.trim().to_string())
      .filter(|value| !value.is_empty())
  } else {
    command_output(program, args)
  }
}

fn parse_bool(value: &str) -> Option<bool> {
  match value {
    "true" => Some(true),
    "false" => Some(false),
    _ => None,
  }
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
  let output = Command::new(program).args(args).current_dir(root_dir()).output().ok()?;
  output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn read_cpus_allowed_list() -> Option<String> {
  let status = fs::read_to_string("/proc/self/status").ok()?;
  status
    .lines()
    .find_map(|line| line.strip_prefix("Cpus_allowed_list:").map(str::trim).map(str::to_string))
}

fn read_cpu_model() -> Option<String> {
  let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
  cpuinfo
    .lines()
    .find_map(|line| line.strip_prefix("model name\t:").map(str::trim).map(str::to_string))
}

fn read_cpu_governors() -> Option<Vec<String>> {
  [0, 2, 4, 6]
    .into_iter()
    .map(|cpu| {
      fs::read_to_string(format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_governor"))
        .ok()
        .map(|governor| format!("{cpu}:{}", governor.trim()))
    })
    .collect()
}

fn read_load_average() -> Option<String> {
  let loadavg = fs::read_to_string("/proc/loadavg").ok()?;
  Some(loadavg.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
}
