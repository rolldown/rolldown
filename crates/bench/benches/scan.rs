use bench::{BenchMode, DeriveOptions, bench_preset, rome_ts_preset, run_bench_group};
use criterion::{Criterion, criterion_group, criterion_main};
use rolldown::BundlerOptions;

fn items() -> Vec<(&'static str, BundlerOptions)> {
  vec![
    ("threejs", bench_preset("threejs", "tmp/bench/three", "entry.js")),
    ("rome_ts", rome_ts_preset()),
    #[cfg(not(feature = "codspeed"))]
    ("threejs10x", bench_preset("threejs", "tmp/bench/three10x", "entry.js")),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  run_bench_group(c, "scan", BenchMode::Scan, &derive_options, items());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
