from pathlib import Path

root = Path.cwd()
import os
if os.environ["BENCH_MODE"] == "walltime":
    p = root / 'Cargo.toml'
    p.write_text(p.read_text().replace(
        'criterion2 = { version = "3.0.4", default-features = false }',
        'criterion = { package = "codspeed-criterion-compat", version = "=6.0.0", default-features = false }',
    ))
    for rel in ['crates/bench/Cargo.toml', 'crates/rolldown_sourcemap/Cargo.toml', 'crates/rolldown_utils/Cargo.toml']:
        p = root / rel
        p.write_text(p.read_text().replace('criterion2', 'criterion').replace('codspeed = ["criterion/codspeed"]', 'codspeed = []'))

# Label the builds without changing benchmark inputs or measured operations.
p = root / 'crates/bench/benches/bench.rs'
p.write_text(p.read_text().replace(
    'let mut group = c.benchmark_group("sourcemap");',
    'let label = std::env::var("WALLTIME_LABEL").expect("validation label");\n  let mut group = c.benchmark_group(format!("{label}/sourcemap"));',
))
p = root / 'crates/bench/src/lib.rs'
p.write_text(p.read_text().replace(
    'let mut group = c.benchmark_group(group_name);',
    'let label = std::env::var("WALLTIME_LABEL").expect("validation label");\n  let mut group = c.benchmark_group(format!("{label}/{group_name}"));',
))
p = root / 'crates/bench/Cargo.toml'
p.write_text(p.read_text() + '\noxc_sourcemap = { workspace = true }\n')
p = root / 'crates/bench/benches/bench.rs'
t = p.read_text()
start = t.index('fn build_chunk_joiner(')
end = t.index('\n}\n', start) + 3
named = t[start:end].replace('fn build_chunk_joiner(', 'fn build_named_chunk_joiner(')
named = named.replace('    // Production', """    let mut parts = map.into_parts();
    let name_id = parts.names.len() as u32;
    parts.names.push("validation_name".into());
    for (index, token) in parts.tokens.iter_mut().enumerate() {
      *token = oxc_sourcemap::Token::new(
        token.get_dst_line(), token.get_dst_col(), token.get_src_line(), token.get_src_col(),
        token.get_source_id(), if index % 3 == 0 { Some(name_id) } else { None },
      );
    }
    parts.token_chunks = None;
    let map = SourceMap::from_parts(parts);
    // Production""", 1)
t = t[:end] + '\n' + named + '\n' + Path('/tmp/concat-guards.rs').read_text() + t[end:]
guards = """  group.bench_function("join_with_named_sourcemap", |b| {
    b.iter_batched_ref(|| build_named_chunk_joiner(BIG_CHUNK), |chunk| black_box(chunk.join()), BatchSize::SmallInput);
  });
  let borrowed_sources = borrowed_chunk_sources(BIG_CHUNK);
  group.bench_function("join_with_borrowed_sourcemap", |b| {
    b.iter_batched_ref(|| borrowed_chunk_joiner(&borrowed_sources), |chunk| black_box(chunk.join()), BatchSize::SmallInput);
  });
  group.bench_function("join_single_sourcemap", |b| {
    b.iter_batched_ref(|| build_chunk_joiner(1), |chunk| black_box(chunk.join()), BatchSize::SmallInput);
  });
"""
t = t.replace('  group.finish();', guards + '  group.finish();')
p.write_text(t)
p = root / 'crates/bench/benches/bench.rs'
t = p.read_text()
start = t.index('fn build_chunk_joiner(')
end = t.index('\n}\n', start) + 3
unmapped = t[start:end].replace('fn build_chunk_joiner(', 'fn build_unmapped_chunk_joiner(')
unmapped = unmapped.replace('    // Production', '''    let mut parts = map.into_parts();
    parts.names.clear();
    for token in parts.tokens.iter_mut() {
      *token = oxc_sourcemap::Token::new(
        token.get_dst_line(), token.get_dst_col(), token.get_src_line(), token.get_src_col(), None, None,
      );
    }
    parts.token_chunks = None;
    let map = SourceMap::from_parts(parts);
    // Production''',1)
large = unmapped.replace('fn build_unmapped_chunk_joiner(module_count: u32)', 'fn build_unmapped_large_chunk_joiner()').replace('0..module_count','0..1').replace('module_source(index)','program_source(BIG_CHUNK)')
t=t[:end]+'\n'+unmapped+'\n'+large+t[end:]
guards='''  group.bench_function("join_fully_unmapped", |b| {
    b.iter_batched_ref(|| build_unmapped_chunk_joiner(BIG_CHUNK), |chunk| black_box(chunk.join()), BatchSize::SmallInput);
  });
  group.bench_function("join_fully_unmapped_large", |b| {
    b.iter_batched_ref(build_unmapped_large_chunk_joiner, |chunk| black_box(chunk.join()), BatchSize::SmallInput);
  });
'''
t=t.replace('  group.finish();', guards+'  group.finish();')
p.write_text(t)
