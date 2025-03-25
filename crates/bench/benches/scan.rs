use bench::{DeriveOptions, derive_benchmark_items};
use criterion::{Criterion, criterion_group, criterion_main};

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("scan");

  let derive_options = DeriveOptions { sourcemap: false, minify: false };

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
  ]
  .into_iter()
  .flatten()
  .collect::<Vec<_>>();

  group.sample_size(20);
  items.into_iter().for_each(|item| {
    group.bench_function(format!("scan@{}", item.name), move |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new((item.options)());
          let _output = rolldown_bundler.scan(vec![]).await.expect("should not failed in scan");
        })
      });
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
