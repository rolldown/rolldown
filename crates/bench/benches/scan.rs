use bench::{DeriveOptions, create_mem_fs_and_resolver, derive_benchmark_items};
use criterion::{Criterion, criterion_group, criterion_main};

use rolldown::BundleFactoryOptions;
use rolldown_common::BundlerOptions;
use rolldown_testing::bundler_options_presets::{rome_ts, threejs};

fn items() -> Vec<(&'static str, BundlerOptions)> {
  vec![
    ("threejs", threejs()),
    ("rome_ts", rome_ts()),
    #[cfg(not(feature = "codspeed"))]
    ("threejs10x", rolldown_testing::bundler_options_presets::threejs10x()),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("scan");
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  items()
    .into_iter()
    .flat_map(|(name, options)| derive_benchmark_items(&derive_options, name, options))
    .for_each(|item| {
      // Preload files into MemoryFileSystem once (outside the timed loop)
      let (mem_fs, resolver) = create_mem_fs_and_resolver(&item.options);

      group.bench_function(format!("scan@{}", item.name), move |b| {
        b.to_async(
          tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .enable_all()
            .max_blocking_threads(4)
            .build()
            .unwrap(),
        )
        .iter(|| {
          let mem_fs = mem_fs.clone();
          let resolver = resolver.clone();
          let options = item.options.clone();
          async move {
            let mut factory =
              rolldown::BundleFactory::new(BundleFactoryOptions {
                bundler_options: options,
                plugins: vec![],
                session: None,
                disable_tracing_setup: true,
              })
              .expect("Failed to create bundle factory");
            let bundle = factory
              .create_bundle_with_fs(mem_fs, resolver)
              .expect("Failed to create bundle");
            bundle.scan().await.expect("should not fail in scan");
          }
        });
      });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
