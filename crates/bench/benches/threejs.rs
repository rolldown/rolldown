use bench::{normalized_fixture_path, run_fixture};
use criterion::{criterion_group, criterion_main, Criterion};

async fn threejs() {
  let fixture_path = normalized_fixture_path("cases/threejs");
  run_fixture(fixture_path).await;
}

async fn threejs10x() {
  let fixture_path = normalized_fixture_path("cases/threejs10x");
  run_fixture(fixture_path).await;
}

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("rolldown benchmark");

  group
    .sample_size(20)
    .bench_function("threejs", |b| {
      b.iter(|| tokio::runtime::Runtime::new().unwrap().block_on(threejs()))
    })
    .bench_function("threejs10x", |b| {
      b.iter(|| {
        tokio::runtime::Runtime::new()
          .unwrap()
          .block_on(threejs10x())
      })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
