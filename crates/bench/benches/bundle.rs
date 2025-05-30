use bench::{DeriveOptions, derive_benchmark_items};
use criterion::{Criterion, criterion_group, criterion_main};
use rolldown_testing::{bundler_options_presets::multi_duplicated_symbol, utils::assert_bundled};

use rolldown_common::BundlerOptions;
use rolldown_testing::bundler_options_presets::{rome_ts, threejs};

fn items() -> Vec<(&'static str, BundlerOptions)> {
  let mut result = vec![
    ("threejs", threejs()),
    ("rome_ts", rome_ts()),
    ("multi_duplicated_symbol", multi_duplicated_symbol()),
  ];
  #[cfg(not(feature = "codspeed"))]
  {
    result.push(("threejs10x", rolldown_testing::bundler_options_presets::threejs10x()));
  }
  result
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("bundle");

  let derive_options = DeriveOptions { sourcemap: true, minify: false };

  items()
    .into_iter()
    .flat_map(|(name, options)| derive_benchmark_items(&derive_options, name, options.clone()))
    .for_each(|item| {
      group.bench_function(format!("bundle@{}", item.name), move |b| {
        b.iter(|| assert_bundled(item.options.clone()));
      });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
