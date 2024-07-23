use bench::{derive_benchmark_items, DeriveOptions};
use criterion::{criterion_group, criterion_main, Criterion};
use rolldown_testing::utils::assert_bundled;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("bundle");

  let derive_options = DeriveOptions { sourcemap: true, minify: true };

  let items = [
    derive_benchmark_items(
      &derive_options,
      "threejs".to_string(),
      rolldown_testing::bundler_options_presets::threejs,
    ),
    derive_benchmark_items(
      &derive_options,
      "threejs10x".to_string(),
      rolldown_testing::bundler_options_presets::threejs10x,
    ),
    derive_benchmark_items(
      &derive_options,
      "rome-ts".to_string(),
      rolldown_testing::bundler_options_presets::rome_ts,
    ),
    derive_benchmark_items(
      &derive_options,
      "multi-duplicated-top-level-symbol".to_string(),
      rolldown_testing::bundler_options_presets::multi_duplicated_symbol,
    ),
  ]
  .into_iter()
  .flatten()
  .collect::<Vec<_>>();

  group.sample_size(20);
  items.into_iter().for_each(|item| {
    group.bench_function(format!("bundle@{}", item.name), move |b| {
      b.iter(|| assert_bundled((item.options)()));
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
