use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use rolldown_sourcemap::SourceJoiner;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("join");
  // A module that is 1kb in size
  let a_norma_module = " ".repeat(1024);

  // `join` takes `&mut self` (it moves each source's map out), so build a fresh
  // joiner per iteration; `iter_batched_ref` also drops it outside the measured
  // region, so the timing is the merge alone — not the per-iteration teardown.
  group.bench_function("join", move |b| {
    b.iter_batched_ref(
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
