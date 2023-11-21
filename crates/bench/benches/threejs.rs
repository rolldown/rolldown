use std::path::PathBuf;

use bench::join_by_repo_root;
use criterion::{criterion_group, criterion_main, Criterion};
use rolldown::InputOptions;

#[derive(Debug)]
struct BenchItem {
  name: &'static str,
  entry_path: PathBuf,
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("rolldown benchmark");

  let items = vec![
    BenchItem { name: "threejs", entry_path: join_by_repo_root("temp/three/entry.js") },
    BenchItem { name: "threejs10x", entry_path: join_by_repo_root("temp/three10x/entry.js") },
  ];
  group.sample_size(20);
  items.into_iter().for_each(|item| {
    let scan_id = format!("{}-scan", item.name);
    let build_id = format!("{}-build", item.name);
    let bundle_id = format!("{}-bundle", item.name);
    group.bench_function(scan_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(InputOptions {
            input: vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }],
            cwd: join_by_repo_root("crates/benches"),
            ..Default::default()
          });
          rolldown_bundler.scan().await.unwrap();
        })
      });
    });
    group.bench_function(build_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(InputOptions {
            input: vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }],
            cwd: join_by_repo_root("crates/benches"),
            ..Default::default()
          });
          rolldown_bundler.build().await.unwrap();
        })
      });
    });
    group.bench_function(bundle_id, |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
          let mut rolldown_bundler = rolldown::Bundler::new(InputOptions {
            input: vec![rolldown::InputItem {
              name: Some(item.name.to_string()),
              import: item.entry_path.to_string_lossy().to_string(),
            }],
            cwd: join_by_repo_root("crates/bench"),
            ..Default::default()
          });
          rolldown_bundler.build().await.unwrap();
          rolldown_bundler.write(Default::default()).await.unwrap();
        })
      });
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
