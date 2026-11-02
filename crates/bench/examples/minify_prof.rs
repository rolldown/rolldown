use bench::{bench_preset, create_bench_context, rome_ts_preset};
use rolldown::{BundlerOptions, SourceMapType};

#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

#[cfg(unix)]
fn process_cpu_ns() -> u64 {
  let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
  // SAFETY: `time` is a valid writable `timespec` for the duration of the call.
  if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &raw mut time) } != 0 {
    return 0;
  }
  (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

#[cfg(not(unix))]
fn process_cpu_ns() -> u64 {
  0
}

/// Peak resident set size of this process in MiB (`getrusage(RUSAGE_SELF).ru_maxrss`).
#[cfg(unix)]
fn max_rss_mib() -> f64 {
  let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
  // SAFETY: `usage` points to writable storage for `rusage` and is only read after success.
  if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
    return 0.0;
  }
  // SAFETY: `getrusage` initialized `usage` when it returned success.
  let max_rss = unsafe { usage.assume_init() }.ru_maxrss as f64;
  // macOS reports bytes; Linux and the other supported Unix targets report KiB.
  let bytes_per_unit = if cfg!(target_vendor = "apple") { 1.0 } else { 1024.0 };
  max_rss * bytes_per_unit / (1024.0 * 1024.0)
}

#[cfg(not(unix))]
fn max_rss_mib() -> f64 {
  0.0
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mut opts: BundlerOptions = match args[1].as_str() {
    "three10x" => bench_preset("three10x", "tmp/bench/three10x", "entry.js"),
    "rome" => rome_ts_preset(),
    other if other.starts_with("rome:") => {
      let mut opts = rome_ts_preset();
      opts.input = Some(vec![rolldown::InputItem {
        name: Some("rome-ts".to_string()),
        import: std::path::Path::new("tmp/bench/rome")
          .join(&other["rome:".len()..])
          .canonicalize()
          .unwrap()
          .to_string_lossy()
          .into_owned(),
      }]);
      opts
    }
    other => {
      let (d, e) = other.split_once(':').unwrap_or((other, "entry.js"));
      bench_preset("x", d, e)
    }
  };
  let iters: usize = args[2].parse().unwrap();
  if std::env::var_os("NO_MAP").is_none() {
    opts.sourcemap = Some(SourceMapType::File);
  }
  match args[3].as_str() {
    "off" => opts.minify = Some(false.into()),
    "full" => opts.minify = Some(true.into()),
    _ => {}
  }
  if std::env::var("PLATFORM").is_ok_and(|p| p == "node") {
    opts.platform = Some(rolldown::Platform::Node);
  }
  let out = args.get(4).cloned();
  let mut ctx = create_bench_context(&opts);
  let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  let p0 = process_cpu_ns();
  let w0 = std::time::Instant::now();
  for i in 0..iters {
    let bundle = ctx.factory.create_bundle_with_fs(ctx.mem_fs.clone(), ctx.create_resolver());
    let output = rt.block_on(bundle.generate()).unwrap();
    if i == 0
      && let Some(dir) = &out
    {
      std::fs::create_dir_all(dir).unwrap();
      for asset in &output.assets {
        let path = std::path::Path::new(dir).join(asset.filename());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, asset.content_as_bytes()).unwrap();
      }
    }
  }
  let wall = w0.elapsed().as_secs_f64() * 1e3;
  let proc_cpu = (process_cpu_ns() - p0) as f64 / 1e6;
  println!(
    "{} {} iters={iters} wall={wall:.0}ms proc_cpu={proc_cpu:.0}ms max_rss={:.0}MiB {}",
    args[1],
    args[3],
    max_rss_mib(),
    rolldown::poc_prof_report()
  );
}
