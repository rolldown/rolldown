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
      opts.module_types =
        Some(FxHashMap::from_iter([("css".to_string(), ModuleType::Empty)]));
      opts
    }),
    #[cfg(not(feature = "codspeed"))]
    ("threejs10x", bench_preset("threejs", "tmp/bench/three10x", "entry.js")),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: true, minify: false };
  run_bench_group(c, "bundle", BenchMode::Bundle, &derive_options, items());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
