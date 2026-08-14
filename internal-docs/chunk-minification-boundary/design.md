# Chunk Minification Boundary — Design & Principles

## Summary

Rolldown currently renders every finalized module AST to text, joins those strings with
format-specific glue, and then asks Oxc to parse the completed chunk before running DCE or full
minification. This boundary is simple and is required after a `renderChunk` hook changes the code,
but it repeats codegen and parsing for the common plugin-free path. The intended optimization is to
preserve a **single chunk-wide AST** through rendering and run Oxc against it before the first
codegen when doing so is unobservable. See [implementation.md](./implementation.md) for the current
pipeline and concrete file ownership.

This is an architectural optimization, not an allocator tuning exercise. Local profiling on a
10 MiB, 6,091-module single chunk found final `dce-only` minification to account for about 18% of
the build's CPU samples. Within that stage, parsing, semantic construction, and codegen accounted
for most of the work. A synthetic Criterion prototype that preserves module ASTs reduced the
isolated boundary from about 1.16 ms to 0.77 ms (roughly 33%), which is enough headroom to justify
the larger design without claiming the same end-to-end speedup.

## Goals

1. Remove module-codegen → chunk-parse duplication when Rolldown controls the complete chunk.
2. Preserve chunk-wide semantic analysis. Scope hoisting creates declarations in one module AST
   that are referenced by another module AST, so DCE must see the whole chunk as one program.
3. Keep plugin ordering, comment policy, sourcemaps, output formats, and minification output
   compatible with the existing string pipeline.
4. Retain the current text path as a correctness fallback rather than widening the fast path with
   assumptions that plugins or generated glue cannot observe.
5. Measure the boundary independently from end-to-end bundling so implementation work has a stable
   regression signal.

## Non-goals

- Replacing Rolldown's link-stage tree shaking with Oxc DCE. Rolldown still decides module and
  statement inclusion from the module graph; Oxc still cleans up the rendered chunk.
- Running Oxc DCE independently on finalized module ASTs. That loses cross-module references after
  scope hoisting and can delete live declarations.
- Moving DCE before `renderChunk` when a render hook exists. Hooks currently observe pre-minified
  code and may deliberately produce code that final DCE removes.
- Changing `minify: "dce-only"` semantics to gain speed.

## Required invariants

### One semantic program per chunk

The fast path must construct one Oxc `Program` and rebuild one `Scoping` for the entire chunk.
Concatenating the results of module-local DCE is not equivalent. For example, a declaration owned
by module A can be referenced only from module B after import/export syntax has been removed; a
semantic build of A alone reports that declaration as unused.

### Plugin visibility is unchanged

`renderChunk` receives the rendered, non-minified string today. Therefore the AST path is eligible
only when no render hook can run. Calling a hook with already-DCE'd output and falling back only
when it returns a replacement is too late: merely changing the hook input is observable.

Banner, intro, outro, footer, format glue, generated imports/exports, hashbangs, and directives are
part of the pre-minification program. They must either be represented in the chunk AST in exact
order or make the first implementation fall back to text.

### Comments and source origins survive

Oxc AST comments and spans refer to a program's source text. Module ASTs have different source
owners and overlapping span ranges, so cloning statements into a chunk allocator is not sufficient
for legal comments or source maps. A first fast path may conservatively require sourcemaps and
retained comments to be disabled. A complete implementation needs an explicit per-segment source
origin model or equivalent codegen support.

Pure annotations are semantically important: the current module codegen forces annotation comments
into the intermediate string so the final parser can reconstruct `pure` flags. An AST-preserving
path should carry the flags directly and must not rely on that textual roundtrip.

### Peak memory remains bounded

The current pipeline drops per-module bump allocators before allocating the final chunk parser AST.
Keeping module arenas and also cloning a chunk AST can increase peak memory substantially. Chunk
AST construction should consume/take module ASTs where possible, drop each source arena promptly,
and limit parallel chunk construction if measurements show excessive peak memory.

## Proposed representation

Introduce an internal render plan that retains ordering without immediately flattening everything
to `String`:

```text
ChunkRenderPlan
  ├─ generated AST/text segments (directives, imports, wrappers, exports)
  ├─ finalized module AST segments in execution order
  ├─ addon segments (banner/intro/outro/footer)
  └─ source-origin and comment metadata
```

The plan supports two materializations:

- **Text materialization:** current behavior and universal fallback; required for `renderChunk`.
- **AST materialization:** clone or move statements into one chunk allocator, parse only the small
  generated text segments that do not yet have AST builders, build chunk-wide semantics, run Oxc,
  and codegen once.

Parsing small glue segments is acceptable in the first implementation. The expensive operation to
avoid is reparsing all rendered module bodies.

## Rollout

1. Keep the Criterion boundary benchmark and require byte-identical output between its two paths.
2. Introduce `ChunkRenderPlan` while still materializing text for all builds. This separates the
   representation change from minifier behavior.
3. Enable AST materialization for a narrow ESM case with no render hooks, addons, sourcemaps, or
   retained comments. Compare all existing fixtures byte-for-byte with the text path.
4. Re-profile a large real project and check both wall time and peak resident memory before
   broadening eligibility.
5. Add generated glue, comments, source origins, other output formats, and full minification one
   contract at a time. Keep an explicit fallback reason so coverage can be measured.

## Rejected experiments

- **Pre-sizing the Oxc allocator from source length:** source length, 2×, and 4× capacities were
  neutral or slower than the default allocator in the isolated DCE benchmark.
- **Disabling JSX parsing for known non-JSX chunks:** no statistically significant improvement in
  50 samples (`p = 0.69`).
- **Replacing `AllocatorPool` with Rayon worker-local allocators:** the existing pool was faster on
  a 32-chunk synthetic workload (about 3.07 ms versus 3.91 ms).
- **Module-local DCE after finalization:** unsafe without preserving every cross-module live binding
  and still does not cover generated chunk glue or post-render plugin transformations.

## Unresolved questions

- Should generated format glue move to Oxc AST builders, or remain small parsed text segments?
- Can Oxc codegen accept per-statement source origins, avoiding one synthetic chunk source text?
- What eligibility metadata belongs on `InstantiatedChunk`, and which fallback reasons should be
  visible in tracing?
- Can module AST ownership be consumed into a chunk representation without cloning, or is a bounded
  clone cheaper than changing Oxc arena ownership?

## Related

- [implementation.md](./implementation.md) — current control flow and implementation map
- [Bundler Data Lifecycle](../bundler-data-lifecycle/implementation.md) — ownership and arena
  lifetime considerations across a build
