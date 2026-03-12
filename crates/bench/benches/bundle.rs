use bench::{DeriveOptions, create_bench_context, derive_benchmark_items};
use criterion::{Criterion, criterion_group, criterion_main};

use rolldown_common::BundlerOptions;
use rolldown_testing::bundler_options_presets::{multi_duplicated_symbol, rome_ts, threejs};

fn items() -> Vec<(&'static str, BundlerOptions)> {
  vec![
    ("threejs", threejs()),
    ("rome_ts", rome_ts()),
    ("multi-duplicated-top-level-symbol", multi_duplicated_symbol()),
    #[cfg(not(feature = "codspeed"))]
    ("threejs10x", rolldown_testing::bundler_options_presets::threejs10x()),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("bundle");

  let derive_options = DeriveOptions { sourcemap: true, minify: false };

  items()
    .into_iter()
    .flat_map(|(name, options)| derive_benchmark_items(&derive_options, name, options))
    .for_each(|item| {
      let mut ctx = create_bench_context(&item.options);

      group.bench_function(format!("bundle@{}", item.name), move |b| {
        b.to_async(
          tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .enable_all()
            .max_blocking_threads(4)
            .build()
            .unwrap(),
        )
        .iter(|| {
          let mem_fs = ctx.mem_fs.clone();
          let resolver = ctx.resolver.clone();
          let bundle = ctx.factory.create_bundle_with_fs(mem_fs, resolver);
          async move {
            let result = bundle.generate().await;
            if let Err(e) = result {
              panic!("Failed to bundle: {e}");
            }
          }
        });
      });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
