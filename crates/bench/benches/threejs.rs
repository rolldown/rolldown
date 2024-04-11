use std::path::PathBuf;

use bench::join_by_repo_root;
use codspeed_criterion_compat::{criterion_group, criterion_main, Criterion};
use rolldown::{BundlerOptions, SourceMapType};

#[derive(Debug)]
struct BenchItem {
  name: &'static str,
  entry_path: PathBuf,
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("rolldown benchmark");

  let items = vec![
    BenchItem { name: "threejs", entry_path: join_by_repo_root("tmp/bench/three/entry.js") },
    BenchItem { name: "threejs10x", entry_path: join_by_repo_root("tmp/bench/three10x/entry.js") },
  ];
  group.sample_size(20);
  items.into_iter().for_each(|item| {
    let scan_id = format!("{}-scan", item.name);
    // let build_id = format!("{}-build", item.name);
    let source_map_id = format!("{}-sourcemap", item.name);
    let bundle_id = format!("{}-bundle", item.name);
    group.bench_function(scan_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(BundlerOptions {
            input: Some(vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }]),
            cwd: join_by_repo_root("crates/benches").into(),
            ..Default::default()
          });
          let errors = rolldown_bundler.scan().await;
          assert!(errors.is_empty(), "failed to bundle: {:?}", errors);
        })
      });
    });
    // group.bench_function(build_id, |b| {
    //   b.iter(|| {
    //     tokio::runtime::Runtime::new().unwrap().block_on(async {
    //       let mut rolldown_bundler = rolldown::Bundler::new(InputOptions {
    //         input: vec![rolldown::InputItem {
    //           name: Some(item.name.to_string()),
    //           import: item.entry_path.to_string_lossy().to_string(),
    //         }],
    //         cwd: join_by_repo_root("crates/benches"),
    //         ..Default::default()
    //       });
    //       rolldown_bundler.build().await.unwrap();
    //     })
    //   });
    // });
    group.bench_function(bundle_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(BundlerOptions {
            input: Some(vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }]),
            cwd: join_by_repo_root("crates/bench").into(),
            ..Default::default()
          });
          let result = rolldown_bundler.write().await;
          assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
        })
      });
    });

    group.bench_function(source_map_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(BundlerOptions {
            input: Some(vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }]),
            cwd: join_by_repo_root("crates/bench").into(),
            sourcemap: Some(SourceMapType::File),
            ..Default::default()
          });
          let result = rolldown_bundler.write().await;
          assert!(result.errors.is_empty(), "failed to bundle: {:?}", result.errors);
        })
      });
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
