use bench::{bench_preset, create_bench_context, rome_ts_preset};
use rolldown::{BundlerOptions, SourceMapType};

#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

fn cpu_ns(clk: i32) -> u64 {
  #[repr(C)]
  struct Ts(i64, i64);
  unsafe extern "C" {
    fn clock_gettime(clk: i32, ts: *mut Ts) -> i32;
  }
  let mut t = Ts(0, 0);
  unsafe { clock_gettime(clk, &raw mut t) };
  (t.0 as u64) * 1_000_000_000 + t.1 as u64
}

/// Peak resident set size of this process in MiB (`getrusage(RUSAGE_SELF).ru_maxrss`).
fn max_rss_mib() -> f64 {
  // `struct rusage`: two `timeval`s (4 longs), then `ru_maxrss` (KiB on Linux), then 13 longs.
  #[repr(C)]
  struct Rusage([i64; 18]);
  unsafe extern "C" {
    fn getrusage(who: i32, usage: *mut Rusage) -> i32;
  }
  let mut r = Rusage([0; 18]);
  unsafe { getrusage(0, &raw mut r) };
  r.0[4] as f64 / 1024.0
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
  if args[3] == "off" {
    opts.minify = Some(false.into());
  }
  if std::env::var("PLATFORM").is_ok_and(|p| p == "node") {
    opts.platform = Some(rolldown::Platform::Node);
  }
  let out = args.get(4).cloned();
  let mut ctx = create_bench_context(&opts);
  let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  let p0 = cpu_ns(2); // CLOCK_PROCESS_CPUTIME_ID
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
  let proc_cpu = (cpu_ns(2) - p0) as f64 / 1e6;
  let fast = std::env::var("ROLLDOWN_CHUNK_AST_DCE").unwrap_or_default();
  println!(
    "{} {} ast_dce={fast} iters={iters} wall={wall:.0}ms proc_cpu={proc_cpu:.0}ms max_rss={:.0}MiB {}",
    args[1],
    args[3],
    max_rss_mib(),
    rolldown::poc_prof_report()
  );
}
