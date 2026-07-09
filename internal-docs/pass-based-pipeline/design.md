# Pass-Based Pipeline — Design & Principles

## Summary

A guiding methodology for structuring bundler-internal pipelines (stage-level dataflow) as passes with a compile-time ownership contract: each pass declares what it reads, owns, seals, and hands onward, and the borrow checker enforces the declaration. This doc defines the contract; there is no implementation.md yet — the first flow that adopts the contract adds it.

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
- The borrow checker enforces the declaration — not comments, not runtime panics.
- The pipeline is a plain function (the driver); its `let`-chain is the pipeline diagram.
- Wrong order does not compile: a step cannot name inputs that do not exist yet.
- Sealed cannot be mutated: the `&mut` cannot be written.

## What it looks like

The whole mechanism is one trait, one marker trait, and one wrapper function:

```rust
pub trait Pass {
  type InputRead<'a>: Copy;         // shared borrows only; Copy makes `&mut` unrepresentable here
  type InputOwned;                  // data taken over (to modify = to own and hand back); `()` if none
  type OutputRead: SealedArtifact;  // minted here, frozen from here on
  type OutputOwned;                 // still-mutable data handed to a later pass

  async fn run(self, cx: &mut PassCtx, read: Self::InputRead<'_>, owned: Self::InputOwned)
    -> BuildResult<(Self::OutputRead, Self::OutputOwned)>;
}

/// Marker for frozen types: no `&mut` accessors, no interior mutability.
/// Each `impl SealedArtifact for X` is a reviewed authorization.
pub trait SealedArtifact {}
```

- `self` carries the pass's configuration; `run` consumes it — a pass is single-shot.
- `PassCtx` is the **single sanctioned `&mut`**: write-only sinks (diagnostics now, devtools trace later). It never contains pipeline data; passes may write it but never read it.
- Slot conventions: empty slot = `()`; a single artifact = the bare type; multiple = a named per-pass struct (`XxxPassInputRead { .. }`, derives `Copy`).
- Driver rules: every `OutputOwned` is consumed by exactly one later `InputOwned` (or explicitly dropped). **Seal order follows reference direction**: seal an artifact only when everything its keys/indices point into is already sealed.

```text
()  ──Split──►  Draft  ──Optimize──►  Draft  ──Seal──►  ChunkGraph (frozen)
                                                             │
                                 ┌──────── &ChunkGraph ──────┤
                                 ▼                           ▼
                            ComputeLinks                AssignNames     ← disjoint slots ⇒ parallel (see Why)
                                 │                           │
                            Links (sealed)             Names (sealed)
```

## Example

Three passes, three roles — the slots mirror what each pass does to the data:

```rust
// 1) Reshape: chunk merging restructures the graph itself, so it owns the graph.
pub struct OptimizeChunks;

#[derive(Clone, Copy)]
pub struct OptimizeChunksInputRead<'a> {
  pub modules: &'a ModuleTable,
  pub metas: &'a IndexVec<ModuleIdx, LinkingMetadata>,
}

impl Pass for OptimizeChunks {
  type InputRead<'a> = OptimizeChunksInputRead<'a>;
  type InputOwned    = DraftChunkGraph;   // it merges chunks and moves modules
  type OutputRead    = ();
  type OutputOwned   = DraftChunkGraph;   // handed back, still mutable

  async fn run(self, _cx: &mut PassCtx, read: Self::InputRead<'_>, mut graph: Self::InputOwned)
    -> BuildResult<((), Self::OutputOwned)> {
    // we own `graph`: mutate freely, internal `par_iter_mut` is fine
    Ok(((), graph))
  }
}

// 2) Seal: the freeze transition is itself a pass; compaction happens here.
pub struct SealChunkGraph;

impl Pass for SealChunkGraph {
  type InputRead<'a> = ();
  type InputOwned    = DraftChunkGraph;
  type OutputRead    = ChunkGraph;        // `impl SealedArtifact for ChunkGraph` lives next to the type
  type OutputOwned   = ();
  // ...
}

// 3) Derive: the most common shape — owns nothing, reads sealed data, mints a new sealed artifact.
pub struct AssignNames;

#[derive(Clone, Copy)]
pub struct AssignNamesInputRead<'a> {
  pub graph: &'a ChunkGraph,
  pub options: &'a NormalizedBundlerOptions,
}

impl Pass for AssignNames {
  type InputRead<'a> = AssignNamesInputRead<'a>;
  type InputOwned    = ();
  type OutputRead    = ChunkNames;        // e.g. IndexVec<ChunkIdx, ..>, sealed at birth
  type OutputOwned   = ();
  // ...
}
```

The driver is a typed `let`-chain (deliberately **not** `Vec<Box<dyn Pass>>` — heterogeneous signatures are the point):

```rust
let ((), graph) = run_pass(OptimizeChunks, &mut cx, optimize_reads, graph).await?;
let (graph, ()) = run_pass(SealChunkGraph, &mut cx, (), graph).await?;
let (names, ()) = run_pass(AssignNames, &mut cx, names_reads, ()).await?;
```

Needing to own more than you reshape is the signal that an artifact should be split out — not a reason to widen the slot.

## Why

Wrong order is a compile error, not a comment:

```rust
let (canon, ()) = run_pass(Deconflict, &mut cx, DeconflictInputRead { names: &names, .. }, ()).await?;
let (names, ()) = run_pass(AssignNames, &mut cx, names_reads, ()).await?;
// error[E0425]: cannot find value `names` in this scope
```

Sealed means sealed:

```rust
graph.add_chunk(chunk);
// error[E0599]: no method named `add_chunk` found for struct `ChunkGraph`
// (mutators exist only on DraftChunkGraph)
```

### Parallelism is provable from signatures alone

Two passes may run concurrently exactly when their `InputOwned`s are disjoint and neither reads the other's output — both facts sit in the slot types, so "is this join safe?" is answered by signatures, not by auditing bodies:

```rust
// ComputeLinks : InputRead = (&ChunkGraph, &SymbolRefDb), InputOwned = ()
// AssignNames  : InputRead = (&ChunkGraph, &Options),     InputOwned = ()
let (links, names) = try_join!(
  run_pass(ComputeLinks, &mut cx_a, (&graph, &symbols), ()),
  run_pass(AssignNames,  &mut cx_b, (&graph, &options), ()),
)?;
// sinks are write-only, so each branch gets its own; the driver merges them in branch order (deterministic)
```

If a join is unsound (shared owned data, or a missing artifact), it does not race — it fails to compile. The pipeline's parallelization opportunities are enumerable by reading the driver.

### Other benefits

The dependency graph is greppable — impact analysis without reading bodies:

```console
$ rg 'symbol_db: &' -g '*.rs'                 # every pass that reads the symbol table
$ rg 'type InputOwned    = DraftChunkGraph'   # every pass that ever owns the graph
```

Each pass is unit-testable by construction: its `InputRead` struct is the exact, minimal fixture spec — no need to build whole stage outputs to test one pass.

Diagnostics carry provenance for free: `run_pass` stamps the emitting pass's name (via `type_name`) on every warning.

## Future directions

- Driver-level `try_join!` of provably-parallel passes, once profiling shows a win worth taking.
- Incremental cache friendliness: explicit inputs are natural dependency keys, sealed artifacts are natural snapshot/hash units, and a pass is a natural recompute unit. To be honest: the contract was **not** designed with incremental builds as a premise, and nothing in it depends on that — it simply does not stand in the way.

## Related

- `implementation.md` — none yet; added by the first flow that adopts the contract.
