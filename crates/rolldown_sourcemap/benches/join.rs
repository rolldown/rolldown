use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rolldown_sourcemap::SourceJoiner;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("join");
  // A module that is 1kb in size
  let a_norma_module = " ".repeat(1024);

  group.bench_function("join", move |b| {
    let mut joiner = SourceJoiner::default();
    for _ in 0..1024 {
      joiner.append_source(a_norma_module.as_str());
    }
    b.iter(move || {
      black_box(joiner.join());
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
