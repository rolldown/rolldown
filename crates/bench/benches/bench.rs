#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

use bench::{
  BenchMode, DeriveOptions, MapMode, bench_preset, omit_map_plugin, rome_ts_preset, run_bench_group,
};
use criterion::{Criterion, criterion_group, criterion_main};
use rolldown::plugin::__inner::SharedPluginable;
use rolldown::{BundlerOptions, ModuleType};
use rustc_hash::FxHashMap;

fn items() -> Vec<(&'static str, BundlerOptions, Vec<SharedPluginable>)> {
  let no_plugins: Vec<SharedPluginable> = vec![];
  vec![
    ("threejs", bench_preset("threejs", "tmp/bench/three", "entry.js"), no_plugins.clone()),
    (
      "threejs-sourcemap",
      {
        let mut opts = bench_preset("threejs", "tmp/bench/three", "entry.js");
        opts.sourcemap = Some(rolldown::SourceMapType::File);
        opts
      },
      no_plugins.clone(),
    ),
    // A transform hook that changes code but returns no sourcemap, with
    // sourcemap output enabled: this is the path commit f6653cb7b optimizes.
    (
      "threejs-omit-map-sourcemap",
      {
        let mut opts = bench_preset("threejs", "tmp/bench/three", "entry.js");
        opts.sourcemap = Some(rolldown::SourceMapType::File);
        opts
      },
      vec![omit_map_plugin(MapMode::Omitted)],
    ),
    (
      "threejs-null-map-sourcemap",
      {
        let mut opts = bench_preset("threejs", "tmp/bench/three", "entry.js");
        opts.sourcemap = Some(rolldown::SourceMapType::File);
        opts
      },
      vec![omit_map_plugin(MapMode::Null)],
    ),
    // Sanity counterpart: same map-omitting transform but WITHOUT sourcemap
    // output. The optimization should not affect this case.
    (
      "threejs-omit-map-no-sourcemap",
      bench_preset("threejs", "tmp/bench/three", "entry.js"),
      vec![omit_map_plugin(MapMode::Omitted)],
    ),
    ("rome_ts", rome_ts_preset(), no_plugins.clone()),
    (
      "multi-duplicated-top-level-symbol",
      {
        let mut opts = bench_preset(
          "multi_duplicated_symbol",
          "tmp/bench/rolldown-benchcases/packages/multi-duplicated-symbols",
          "index.jsx",
        );
        opts.module_types = Some(FxHashMap::from_iter([("css".to_string(), ModuleType::Empty)]));
        opts
      },
      no_plugins.clone(),
    ),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  run_bench_group(c, "bundle", BenchMode::Bundle, &derive_options, items());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
