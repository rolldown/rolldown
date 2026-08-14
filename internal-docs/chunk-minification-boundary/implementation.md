# Chunk Minification Boundary — Implementation

> The rationale, invariants, and rejected alternatives live in
> [design.md](./design.md).

## Current pipeline

```text
finalize_modules (module ASTs, separate arenas)
  → NormalModule::render / Oxc codegen (one String per module)
  → format renderer + SourceJoiner (one String per chunk)
  → renderChunk hooks (optional String replacement + sourcemap)
  → minify_chunks
      → Oxc parse (new chunk allocator)
      → SemanticBuilder + DCE, or full compress/mangle
      → Oxc codegen
      → sourcemap collapse
  → postBanner/postFooter and final assets
```

The boundary is intentional today: format renderers and plugin hooks produce text. It also gives
Oxc one complete program, so references created by scope hoisting resolve across original module
boundaries.

## Component map

| Responsibility                        | File                                                                                                      | Notes                                                                       |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Finalize scope-hoisted module ASTs    | `crates/rolldown/src/stages/generate_stage/finalize_modules.rs`                                           | Mutates module ASTs in parallel.                                            |
| Codegen module ASTs                   | `crates/rolldown/src/stages/generate_stage/render_chunk_to_assets.rs` (`create_chunk_to_codegen_ret_map`) | Last owner of `IndexEcmaAst`; arenas are currently dropped after this step. |
| Preserve pure annotations across text | `crates/rolldown_common/src/module/normal_module.rs` (`NormalModule::render`)                             | Forces annotation comments into intermediate code for the final parser.     |
| Join module code and generated glue   | `crates/rolldown/src/ecmascript/ecma_generator.rs` and `crates/rolldown/src/ecmascript/format/*.rs`       | Produces a `SourceJoiner`, then one chunk string and optional map.          |
| Invoke output plugins                 | `crates/rolldown/src/utils/render_chunks.rs`                                                              | `renderChunk` can replace arbitrary code and supply another map.            |
| Dispatch chunk minification           | `crates/rolldown/src/stages/generate_stage/minify_chunks.rs`                                              | Parallel across chunks; one large chunk remains serial internally.          |
| Parse, minify, and print              | `crates/rolldown_ecmascript/src/ecma_compiler.rs` (`dce_or_minify`)                                       | The text → AST → text roundtrip.                                            |
| Assemble strings and maps             | `crates/rolldown_sourcemap/src/source_joiner.rs`                                                          | Already preallocates the destination string and has a map-less fast path.   |

## Benchmark

`crates/bench/benches/bench.rs` contains the `chunk_dce` Criterion group.

- `single_chunk_text_roundtrip` measures parse + DCE + codegen for one large rendered program.
- `boundary_text_roundtrip` starts with 256 already-parsed module ASTs, codegens every module,
  joins their text, reparses the chunk, runs DCE, and codegens the result.
- `boundary_ast_clone` clones the same module statements into one allocator, runs chunk-wide DCE,
  and codegens once.

The benchmark asserts byte-identical output before timing the two boundary variants. The AST case is
a feasibility lower bound, not production code: it deliberately omits comments, generated format
glue, plugin hooks, and sourcemaps.

Run only this group with:

```sh
cargo bench -p bench --bench bench -- boundary_
```

The final 2026-08-14 Apple M5 run (30 samples after a 2-second warmup) measured approximately
1.159 ms for `boundary_text_roundtrip` and 0.771 ms for `boundary_ast_clone`, a 33.5% reduction.
Treat the ratio as the signal; absolute values depend on host load and build configuration.

## Fast-path eligibility sketch

The first production experiment should require all of the following:

- output format is ESM;
- no plugin has a `renderChunk` hook;
- no banner/intro/outro/footer or equivalent pre-minification addon can inject text;
- sourcemaps are disabled;
- output comment settings require no source-backed comments;
- no renderer feature still emits unrepresented text that cannot be parsed as a small glue segment.

Eligibility must be decided before invoking hooks because hook input is observable. If any
condition fails, materialize the existing text pipeline without changing output.

## Validation requirements

Before enabling the path outside a benchmark:

1. Add a regression fixture where module B is the only user of a declaration owned by module A;
   module-local DCE must not delete it.
2. Run the existing tree-shake `renderChunk` fixture to verify DCE remains after hook replacement.
3. Compare fast and fallback outputs byte-for-byte across the Rust and Node fixture suites.
4. Re-run the large single-chunk benchmark with `minify: false`, `"dce-only"`, and `true`.
5. Record peak RSS as well as time; the new chunk arena must not silently trade speed for excessive
   memory.

## Related

- [design.md](./design.md) — goals, invariants, rollout, and rejected experiments
