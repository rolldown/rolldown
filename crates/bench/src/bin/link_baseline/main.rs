use std::{env, fs, path::PathBuf, process::ExitCode};

use bench::link_baseline::{
  BaselineConfig, BaselineMode, require_runner_environment, run_baseline,
  run_baseline_with_trace_layer,
  trace::{LinkTraceLayer, records_link_trace_metadata},
  workloads::baseline_workload,
};
use tracing_subscriber::{Layer as _, filter::filter_fn, prelude::*};

#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

struct Args {
  mode: BaselineMode,
  workload: String,
  warmups: usize,
  samples: usize,
  iterations_per_sample: usize,
  development: bool,
  output: Option<PathBuf>,
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("link-baseline: {error}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), String> {
  if env::args().skip(1).any(|argument| matches!(argument.as_str(), "-h" | "--help")) {
    println!("{}", usage());
    return Ok(());
  }
  let args = parse_args()?;
  let canonical = !args.development;
  require_runner_environment(args.mode, canonical)?;
  let trace_layer = if args.mode == BaselineMode::LinkTrace {
    let layer = LinkTraceLayer::default();
    let subscriber = tracing_subscriber::registry()
      .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
    tracing::subscriber::set_global_default(subscriber).map_err(|error| {
      format!(
        "link-trace mode requires a fresh process without an existing global tracing subscriber: {error}"
      )
    })?;
    Some(layer)
  } else {
    None
  };
  let workload = baseline_workload(&args.workload)?;
  let config = BaselineConfig {
    mode: args.mode,
    warmups: args.warmups,
    samples: args.samples,
    iterations_per_sample: args.iterations_per_sample,
    canonical,
  };
  let runtime = if args.mode == BaselineMode::LinkTrace {
    tokio::runtime::Builder::new_current_thread().enable_all().build()
  } else {
    tokio::runtime::Builder::new_multi_thread().worker_threads(8).enable_all().build()
  }
  .map_err(|error| format!("failed to create Tokio runtime: {error}"))?;
  let report = match trace_layer.as_ref() {
    Some(layer) => runtime.block_on(run_baseline_with_trace_layer(config, workload, layer))?,
    None => runtime.block_on(run_baseline(config, workload))?,
  };
  let json = serde_json::to_string(&report)
    .map_err(|error| format!("failed to serialize report: {error}"))?;
  if let Some(path) = args.output {
    if let Some(parent) = path.parent()
      && !parent.as_os_str().is_empty()
    {
      fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&path, format!("{json}\n"))
      .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
  } else {
    println!("{json}");
  }
  Ok(())
}

fn parse_args() -> Result<Args, String> {
  let mut mode = None;
  let mut workload = None;
  let mut warmups = None;
  let mut samples = None;
  let mut iterations_per_sample = None;
  let mut development = false;
  let mut output = None;
  let mut args = env::args().skip(1);

  while let Some(flag) = args.next() {
    match flag.as_str() {
      "--mode" => mode = Some(BaselineMode::parse(&next_value(&mut args, "--mode")?)?),
      "--workload" => workload = Some(next_value(&mut args, "--workload")?),
      "--warmups" => {
        warmups = Some(parse_usize(&next_value(&mut args, "--warmups")?, "--warmups")?)
      }
      "--samples" => {
        samples = Some(parse_usize(&next_value(&mut args, "--samples")?, "--samples")?)
      }
      "--iterations-per-sample" => {
        iterations_per_sample = Some(parse_usize(
          &next_value(&mut args, "--iterations-per-sample")?,
          "--iterations-per-sample",
        )?)
      }
      "--development" => development = true,
      "--output" => output = Some(PathBuf::from(next_value(&mut args, "--output")?)),
      "-h" | "--help" => unreachable!("help is handled before argument parsing"),
      _ => return Err(format!("unknown argument `{flag}`\n{}", usage())),
    }
  }

  let mode = mode.ok_or_else(|| format!("missing --mode\n{}", usage()))?;
  let workload = workload.ok_or_else(|| format!("missing --workload\n{}", usage()))?;
  let (default_warmups, default_samples, default_iterations_per_sample) = defaults(mode, &workload);
  Ok(Args {
    mode,
    workload,
    warmups: warmups.unwrap_or(default_warmups),
    samples: samples.unwrap_or(default_samples),
    iterations_per_sample: iterations_per_sample.unwrap_or(default_iterations_per_sample),
    development,
    output,
  })
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
  args.next().ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_usize(value: &str, flag: &str) -> Result<usize, String> {
  value.parse().map_err(|_| format!("{flag} requires a non-negative integer, got `{value}`"))
}

fn defaults(mode: BaselineMode, workload: &str) -> (usize, usize, usize) {
  match mode {
    BaselineMode::LinkTrace => (2, 10, 1),
    BaselineMode::Digest | BaselineMode::LinkRss | BaselineMode::ScanRss => (0, 1, 1),
    BaselineMode::LinkTime if workload == "overhead-64" => (100, 500, 200),
    BaselineMode::LinkTime => (10, 50, link_iterations_per_sample(workload)),
    BaselineMode::BundleTime if workload == "overhead-64" => (100, 500, 1),
    BaselineMode::BundleTime => (10, 50, 1),
  }
}

fn link_iterations_per_sample(workload: &str) -> usize {
  match workload {
    "dynamic-1024" => 16,
    "wide-4096" | "deep-1024" | "scc-256x4" | "export-star-1024" | "cjs-2048" | "three-r108" => 32,
    "rome" => 64,
    "json-2048" => 160,
    _ => 32,
  }
}

fn usage() -> &'static str {
  "usage: link-baseline --mode <link-time|link-trace|digest|link-rss|scan-rss|bundle-time> --workload <id> [--warmups N] [--samples N] [--iterations-per-sample N] [--development] [--output PATH]"
}
