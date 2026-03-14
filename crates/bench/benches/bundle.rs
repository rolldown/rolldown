use bench::{BenchMode, DeriveOptions, bench_preset, rome_ts_preset, run_bench_group};
use criterion::{Criterion, criterion_group, criterion_main};
use rolldown::{BundlerOptions, ModuleType};
use rustc_hash::FxHashMap;

fn items() -> Vec<(&'static str, BundlerOptions)> {
  vec![
    ("threejs", bench_preset("threejs", "tmp/bench/three", "entry.js")),
    ("rome_ts", rome_ts_preset()),
    ("multi-duplicated-top-level-symbol", {
      let mut opts = bench_preset(
        "multi_duplicated_symbol",
        "tmp/bench/rolldown-benchcases/packages/multi-duplicated-symbols",
        "index.jsx",
      );
      opts.module_types = Some(FxHashMap::from_iter([("css".to_string(), ModuleType::Empty)]));
      opts
    }),
    #[cfg(not(feature = "codspeed"))]
    ("threejs10x", bench_preset("threejs", "tmp/bench/three10x", "entry.js")),
  ]
}

fn bench_scan(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  run_bench_group(c, "scan", BenchMode::Scan, &derive_options, items());
}

fn bench_link(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  run_bench_group(c, "link", BenchMode::Link, &derive_options, items());
}

fn bench_generate(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: true, minify: true };
  run_bench_group(c, "generate", BenchMode::Generate, &derive_options, items());
}

fn bench_bundle(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: true, minify: true };
  run_bench_group(c, "bundle", BenchMode::Bundle, &derive_options, items());
}

criterion_group!(benches, bench_scan, bench_link, bench_generate, bench_bundle);
criterion_main!(benches);
