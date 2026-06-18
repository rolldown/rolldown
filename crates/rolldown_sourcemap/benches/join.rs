use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use rolldown_sourcemap::SourceJoiner;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("join");
  // A module that is 1kb in size
  let a_norma_module = " ".repeat(1024);

  // `join` consumes the joiner, so build a fresh one per iteration (excluded from the measured region).
  group.bench_function("join", move |b| {
    b.iter_batched(
      || {
        let mut joiner = SourceJoiner::default();
        for _ in 0..10_000 {
          joiner.append_source(a_norma_module.clone());
        }
        joiner
      },
      |joiner| black_box(joiner.join()),
      BatchSize::SmallInput,
    );
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
