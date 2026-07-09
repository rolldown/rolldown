# Pass-Based Pipeline — Design & Principles

## Summary

A guiding methodology for structuring bundler-internal pipelines (stage-level dataflow) as passes with a compile-time ownership discipline: each pass is a plain typed function that declares what it reads, owns, seals, and hands onward, and ordinary Rust — by-value parameters, the driver's `let`-chain, distinct draft/frozen artifact types — enforces the declaration. This doc defines the discipline; there is no implementation.md yet — the first flow that adopts it adds one.

## Ground rules (read this first)

- This is a methodology, **not a migration mandate**. Do **not** proactively refactor existing pipeline code into passes.
- Use the pass mechanism only when a maintainer explicitly asks for a pass-based refactor of a flow, or when designing a **new** flow.
- It applies to pipeline top-level structure only. Helpers, visitors, and utilities stay plain functions — do not try to make everything a pass.
- Granularity test: if you cannot name the artifact a step produces (or the working data it transforms), it is not a pass.

## What it is

- A pass is one pipeline step with a machine-checked contract.
- The contract declares four things:
  - what it only reads
  - what it takes ownership of
  - what it seals forever
  - what it hands onward, still mutable
- Enforcement comes from ordinary Rust, not from a framework:
  - the driver's `let`-chain encodes order — a step cannot name inputs that do not exist yet
  - distinct draft/frozen artifact types encode sealing — the frozen type simply has no mutators
  - by-value parameters encode "to modify is to own and hand back"
- There is deliberately no shared `Pass` trait yet — see [Deferred: a shared Pass trait](#deferred-a-shared-pass-trait).

## What it looks like

A pass is a plain async function with a disciplined signature:

```rust
pub async fn optimize_chunks(
  cx: &mut PassCtx,                  // the single sanctioned `&mut`: write-only sinks
  read: OptimizeChunksInput<'_>,     // shared borrows only
  graph: DraftChunkGraph,            // owned: the data this pass reshapes
) -> BuildResult<DraftChunkGraph>    // handed back, still mutable
```

Conventions:

- Read parameters are shared borrows. One or two stay plain parameters; more become a named per-pass struct (`XxxInput<'a>`, all-`&` fields), which doubles as the pass's greppable dependency manifest.
- To modify is to own: mutable working data is taken by value and handed back, never `&mut`.
- Sealing is a type transition (`DraftChunkGraph → ChunkGraph`): the frozen type exposes no mutators and holds no interior mutability. **Immutability lives in the artifact type's API — nothing else grants it.**
- `PassCtx` is the single sanctioned `&mut`: write-only sinks (diagnostics now, devtools trace later). It never contains pipeline data; passes may write it but never read it.
- Driver rules: every still-mutable output is consumed by exactly one later pass (or explicitly dropped). **Seal order follows reference direction**: seal an artifact only when everything its keys/indices point into is already sealed.

The freeze boundary proven in-tree today is `UsedSymbolRefsBuilder::seal()` in the generate stage: source liveness becomes read-only while chunk layout stays mutable — freeze lines are per-artifact, not global:

```text
UsedSymbolRefsBuilder ──(mutated through chunk generation)──► seal() ──► UsedSymbolRefs (frozen)
                                                (chunk graph stays mutable past this point)
```

What a fully adopted flow could look like — **hypothetical**: today's `GenerateStage` does not have this boundary (its graph keeps being mutated by link derivation, wrapping, naming, and finalization well past chunk optimization). Sealing the chunk graph itself is left to the first flow that adopts this methodology:

```text
()  ──Split──►  Draft  ──Optimize──►  Draft  ──Seal──►  ChunkGraph (frozen)
                                                             │
                                 ┌──────── &ChunkGraph ──────┤
                                 ▼                           ▼
                            ComputeLinks                AssignNames
                                 │                           │
                            Links (sealed)             Names (sealed)
```

## Example

Three passes, three roles — the signatures mirror what each pass does to the data (names are illustrative; `DraftChunkGraph`, `ChunkNames` etc. do not exist today):

```rust
// 1) Reshape: chunk merging restructures the graph itself, so it owns the graph.
#[derive(Clone, Copy)]
pub struct OptimizeChunksInput<'a> {
  pub modules: &'a ModuleTable,
  pub metas: &'a IndexVec<ModuleIdx, LinkingMetadata>,
}

pub async fn optimize_chunks(
  cx: &mut PassCtx,
  read: OptimizeChunksInput<'_>,
  mut graph: DraftChunkGraph,
) -> BuildResult<DraftChunkGraph> {
  // we own `graph`: mutate freely, internal `par_iter_mut` is fine
  Ok(graph)
}

// 2) Seal: the freeze transition is itself a pass; compaction happens here.
//    Immutability comes from `ChunkGraph`'s API: it exposes no mutators.
pub fn seal_chunk_graph(graph: DraftChunkGraph) -> ChunkGraph {
  /* compact, re-index, freeze */
}

// 3) Derive: the most common shape — owns nothing, reads sealed data, mints a new sealed artifact.
pub async fn assign_names(
  cx: &mut PassCtx,
  graph: &ChunkGraph,
  options: &NormalizedBundlerOptions,
) -> BuildResult<ChunkNames>
```

The driver is a typed `let`-chain; the chain is the pipeline diagram:

```rust
let graph = optimize_chunks(&mut cx, optimize_input, graph).await?;
let graph = seal_chunk_graph(graph);
let names = assign_names(&mut cx, &graph, &options).await?;
```

Needing to own more than you reshape is the signal that an artifact should be split out — not a reason to widen a parameter.

## Why

Wrong order is a compile error, not a comment:

```rust
let canon = deconflict(&mut cx, DeconflictInput { names: &names, /* .. */ }).await?;
let names = assign_names(&mut cx, &graph, &options).await?;
// error[E0425]: cannot find value `names` in this scope
```

Sealed means sealed:

```rust
graph.add_chunk(chunk);
// error[E0599]: no method named `add_chunk` found for struct `ChunkGraph`
// (mutators exist only on DraftChunkGraph)
```

### Parallelism: signatures expose the candidates, the compiler checks the join

Two passes are concurrency candidates exactly when their owned inputs are disjoint and neither reads the other's output — both facts sit in the signatures. What each layer actually guarantees:

- **Signatures** expose the candidates: disjoint owned data, no artifact dependency between the two.
- **The borrow checker (plus `Send`/`Sync`)** proves the join is free of data races — an unsound join (shared owned data, missing artifact) fails to compile.
- **Semantic independence is not proven; it is a stated discipline**: no interior mutability in pipeline data, no globals, no order-dependent external calls (plugin hooks, I/O) inside candidate passes, and effects only through per-branch `PassCtx` sinks that the driver merges in a fixed order. Under those rules — and only under them — a compiling join is also deterministic.

```rust
// compute_links: reads (&ChunkGraph, &SymbolRefDb), owns nothing
// assign_names : reads (&ChunkGraph, &Options),     owns nothing
let (links, names) = try_join!(
  compute_links(&mut cx_a, &graph, &symbols),
  assign_names(&mut cx_b, &graph, &options),
)?;
```

### Other benefits

The dependency graph is greppable — impact analysis without reading bodies:

```console
$ rg 'symbol_db: &' -g '*.rs'          # every pass that reads the symbol table
$ rg 'graph: DraftChunkGraph'          # every pass that ever owns the draft graph
```

Each pass is unit-testable by construction: its read parameters (or `XxxInput` struct) are the exact, minimal fixture spec — no need to build whole stage outputs to test one pass.

## Deferred: a shared Pass trait

An earlier draft packaged the four slots as a trait (`type InputRead<'a>: Copy` / `InputOwned` / `OutputRead: SealedArtifact` / `OutputOwned`, plus `async fn run(self, cx, read, owned)`). It is deliberately **not** part of the contract yet:

- Every property in [Why](#why) already comes from plain signatures, the `let`-chain, and distinct draft/frozen types — the trait adds GATs and `()`/tuple ceremony without adding enforcement.
- What it would buy is uniformity: one `run_pass` wrapper as the single home for tracing spans and diagnostics provenance, and a pinned signature shape (the `Copy` bound on reads mechanically rejects `&mut`). What it cannot do: encode pass order, freeze outputs, or make dependencies exhaustive — `self` could still smuggle pipeline state unless separately restricted.
- A `SealedArtifact` marker likewise enforces nothing: Rust only checks that an impl exists. If ever introduced, it is a **reviewed inventory** of frozen types — the immutability itself always lives in each artifact type's API.
- Adoption trigger: several adopted flows showing repetition that ordinary functions cannot express. Until then, plain functions.

## Future directions

- Driver-level `try_join!` of parallel-candidate passes, once profiling shows a win worth taking.
- Incremental cache friendliness: explicit inputs are natural dependency keys, sealed artifacts are natural snapshot/hash units, and a pass is a natural recompute unit. To be honest: the contract was **not** designed with incremental builds as a premise, and nothing in it depends on that — it simply does not stand in the way.

## Related

- `implementation.md` — none yet; added by the first flow that adopts the contract.
