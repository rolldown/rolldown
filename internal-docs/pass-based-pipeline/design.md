# Pass-Based Pipeline — Design & Principles

## Summary

A pass-based pipeline makes a stage's dataflow visible in Rust types: each top-level step declares the facts it reads, the working sets it owns, the fact it mints for shared reading, the owned data it hands onward, and its error type. The harness controls execution with a branded capability, seals every read-side output, owns diagnostic provenance, and leaves pass order as an ordinary typed driver. Link is the first maintainer-selected adoption: the production driver now runs twenty-four narrow passes, ending with typed tree shaking and dependency finalization before a one-shot legacy output adapter reconstructs Generate's unchanged boundary. Derived link facts no longer live in `LinkStage` or a preallocated metadata carrier between passes. The implemented harness, current passes, and unchanged boundary are described in [implementation.md](./implementation.md).

## Ground rules

- This is a methodology, not a migration mandate. Refactor an existing flow only when a maintainer selects it, or use it while designing a new flow.
- A pass is a synchronous top-level driver step that produces a named artifact or transforms a named working set. Helpers, visitors, recursion frames, and per-entity loops remain ordinary functions.
- A pass is not a renamed stage method. If its contract needs a stage object, a metadata grab bag, or an equivalent replacement state bag, the dataflow has not been separated.
- A pass token is a non-generic unit struct with no runtime data. Configuration and pipeline data enter through declared slots.
- A typed `let`-chain or a small statically visible DAG is the driver. A `Vec<Box<dyn Pass>>` would erase the heterogeneous contracts that make the design useful.

## Contract

Every pass declares five associated types:

- `InputRead<'a>` contains copyable values and shared views of already-existing facts.
- `InputOwned` moves in data that the pass may reshape or consume.
- `OutputRead` is a purpose-specific fact minted by the pass and returned as `Sealed<OutputRead>`.
- `OutputOwned` moves still-owned data back to the driver.
- `Error` is the pass-specific failure channel; an infallible pass uses `Infallible`.

The implemented public signatures are:

```rust
pub trait Pass: Sized + Copy + 'static {
  type InputRead<'a>: Copy;
  type InputOwned: 'static;
  type OutputRead: 'static;
  type OutputOwned: 'static;
  type Error: 'static;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error>;
}

pub type PassOutput<P> = (Sealed<<P as Pass>::OutputRead>, <P as Pass>::OutputOwned);

pub type PassResult<P> = Result<PassOutput<P>, <P as Pass>::Error>;

pub fn run_pass<P: Pass>(
  pass: P,
  pipeline: &mut PassPipelineCtx,
  read: P::InputRead<'_>,
  owned: P::InputOwned,
) -> PassResult<P>;

pub fn run_infallible_pass<P: Pass<Error = Infallible>>(
  pass: P,
  pipeline: &mut PassPipelineCtx,
  read: P::InputRead<'_>,
  owned: P::InputOwned,
) -> PassOutput<P>;
```

`BuildResult` is deliberately not built into the trait. A fallible flow selects its own associated error type and calls `run_pass`; an infallible flow calls `run_infallible_pass`, which eliminates `Infallible` by exhaustive matching and does not add `unwrap`, `expect`, or a panic path.

### Execution authority

A public trait method cannot be made callable only by one free function through visibility alone. The harness therefore controls the capability required to execute it successfully:

```rust
struct RunBrand;

pub struct RunToken<'run, P> {
  _brand: &'run mut RunBrand,
  _lifetime: PhantomData<fn(&'run mut ()) -> &'run mut ()>,
  _pass: PhantomData<fn(P) -> P>,
}

impl<P> RunToken<'_, P> {
  pub fn finish<R, O>(self, read: R, owned: O) -> RawPassOutput<R, O> {
    RawPassOutput { read, owned }
  }
}

pub struct RawPassOutput<R, O> {
  read: R,
  owned: O,
}
```

The fields of `RunToken` and `RawPassOutput` are private. `run_pass` creates a local `RunBrand`, lends it mutably into an invariant `RunToken<'_, P>` for that call, and creates a temporary `PassCtx`. A pass can produce the required envelope only by consuming that token through `finish`. The harness alone can open the envelope, wrap its read-side value in `Sealed`, and return the owned side.

The branded lifetime matters. A token containing only `PhantomData<P>` could be returned as a pass's error or output and reused in a direct call. Here `InputOwned`, `OutputRead`, `OutputOwned`, and `Error` are all `'static`, while the token borrows a harness-local brand for `'run`; safe code cannot leak it through those channels, a `'static` panic payload, or a spawned `'static` task. The pass parameter in the token also prevents using a token for pass `P` to invoke pass `Q`.

`Pass::run` remains a public, nameable method because consuming crates must implement the public trait. The precise guarantee is that safe reachable code outside the harness cannot begin a successful invocation without a harness-issued token, and cannot obtain the successful invocation's designated `OutputRead` except through `Sealed`. The mechanism does not try to stop a pass from forwarding its live token to its own helper while that one invocation is active.

### Sealed and owned outputs

```rust
pub struct Sealed<T>(T);

impl<T> Deref for Sealed<T> {
  type Target = T;

  fn deref(&self) -> &T {
    &self.0
  }
}
```

`Sealed<T>` has no public constructor, mutable dereference, or unwrap. `run_pass` is the only code that constructs it, and only shared access to `T` is exposed. `OutputRead = ()` therefore returns `Sealed<()>`; callers discard that side with `_`, not a literal `()` pattern.

Sealing is not decomposition. `Sealed<LinkingMetadata>` would still be a heterogeneous state bag, and `Sealed<T>` does not remove shared-mutation APIs already present on `T`. It also cannot prevent a caller from cloning a separate unsealed `T` through shared dereference when `T: Clone`; artifact types whose identity matters should not expose that escape. Purpose-specific artifact types remain mandatory. When finalization should narrow the domain API or representation, use a draft/final type pair in addition to the wrapper; fixed representations such as `Box<[T]>` and `IndexBox` are useful when they reflect a real finalized shape.

`OutputOwned` is intentionally not sealed. The driver receives ownership, may lend shared references to later passes, moves it into the next mutator when necessary, and eventually moves it into a boundary adapter or drops it explicitly. Mutation requires ownership, but an owned result does not need to be threaded through unrelated passes just to remain alive.

The resolved-export pair is a concrete boundary case. `CollectResolvedExportsPass` returns owned `ResolvedExportsDraft` because binding must borrow its raw maps and finalization must later consume them. `FinalizeResolvedExportsPass` returns owned `ResolvedExports` because both the raw maps and sorted canonical view ultimately belong to Generate through legacy metadata. Putting either table in `OutputRead` would force a general unsealing escape or a graph-sized clone at the compatibility boundary. The owned artifacts instead expose narrow read APIs and one consuming slot conversion, and the driver moves each final map exactly once after its last Link reader.

### Context and diagnostics

`PassPipelineCtx` belongs to the serial driver or to one parallel branch. `run_pass` derives a temporary `PassCtx` for one invocation and records the concrete pass type in the tracing span and on each diagnostic emission. A pass receives only `&mut PassCtx`; its public surface has `push` and `extend`, with no constructor, getters, drain method, `Default`, or access to pipeline data.

Parallel branches each own a `PassPipelineCtx`. The driver appends completed branch contexts in declared pass order, preserving both each branch's internal diagnostic order and the pipeline's deterministic order. `into_diagnostics` consumes the context, emits provenance to tracing, and returns the ordinary diagnostics collection expected by existing stages.

`PassCtx` is the only sanctioned shared mutable parameter in a pass signature. Adding pipeline state or read access to it would recreate the hidden dependency channel this design removes.

## Driver shape and artifact lifetime

The driver uses ordinary locals, so availability expresses order and Rust moves express ownership:

```rust
let (_, graph) = run_infallible_pass(OptimizeChunksPass, &mut pipeline, optimize_reads, graph);
let (graph, ()) = run_infallible_pass(SealChunkGraphPass, &mut pipeline, (), graph);
let (names, ()) = run_infallible_pass(
  AssignNamesPass,
  &mut pipeline,
  AssignNamesPassInputRead { graph: &graph, options: &options },
  (),
);
```

Trying to use `names` before its producer is an ordinary unresolved-name error. Trying to move one owned working set into two branches is an ordinary move or borrow error. Dropping an artifact too early is caught when the next borrower tries to use the moved value.

Seal order follows reference direction: finalize an artifact only after the identity layout behind every index or reference it stores is stable for the artifact's remaining lifetime. `ModuleIdx`, `StmtInfoIdx`, and `NodeId` do not become stable merely because a containing type is named `Final`.

Memory release points belong in the driver. The last mutator can consume an owned input and omit it from `OutputOwned`; a lent artifact gets an explicit `drop` after its last borrower. Representation compaction and deferred destruction are separate, measured choices rather than implicit effects of the pass abstraction.

## Link boundary and the compatibility adapter

Link is the first selected adoption, but its external boundary remains unchanged: `LinkStage::link` stays infallible and returns the existing `(LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder)` tuple. Generate continues mutating that output, so this migration must not claim that the whole link result is immutable.

`Infallible` describes Link's control-flow boundary, not the absence of internal invariant failures in every transitively called algorithm. `run_infallible_pass` does not adapt an error: it eliminates the uninhabited error by exhaustive matching, while `LinkStage::link` and `bundle_up`'s direct Link initializer add no `Result`, `?`, `unwrap`, `expect`, or panic-based error path. The predecessor Link implementation already used local `Option` short-circuiting and fail-closed invariant checks inside its algorithms. Extracted passes retain those semantics, and artifact or adapter checks reject malformed layouts, ownership, identities, and deferred-patch targets before they can become partial legacy output. Replacing those internal checks with diagnostics, skips, or defaults would change the boundary or permit corrupted output.

The final link driver has a one-shot input adapter, typed passes, and one legacy output adapter. Neither adapter is a pass, and neither aggregate is allowed back into the pass DAG. `LinkStage` contains exactly the untouched scan output and options; `.link()` consumes both facade fields in its first statement and every Scan field in its second, so the facade cannot survive into a pass. The output adapter consumes final link artifacts once, accepts the remaining Scan-owned final fields explicitly rather than through a replacement input bag, and constructs the existing mutable output types. In the completed cutover, initialization traversals live in `ExtractGlobalConstantsPass`, `CanonicalizeEntriesPass`, `CollectInitialDependenciesPass`, and `CollectExternalStarExportsPass`. `ComputeModuleExecutionOrderPass` borrows the unique owned entry draft and returns sealed dense orders plus an owned sorted list. `ComputeTlaPass` consumes its two scan-only fields and returns a sealed compact fact. `DetermineModuleFormatsPass` also borrows the entry draft, produces format and wrapper-seed drafts, and feeds CJS namespace merging, sealed dynamic-export analysis, and wrapper planning without reading a legacy projection. `CreateWrapperDeclarationsPass` allocates paired wrapper identities, then `NormalizeLazyExportsPass` owns the six entity and draft domains that must change atomically and produces final formats and wrappers after the last local-identity invalidator. `DetermineModuleSideEffectsPass` then borrows only the module table, sealed dynamic exports, and final wrappers; it preserves the legacy serial cache semantics and emits a sealed dense enum table. `CollectResolvedExportsPass` reads only the final module table, keeps independent physical roots parallel while preserving serial path-local DFS order, and returns an owned dense draft.

The driver retains `ModuleDependenciesDraft`, sealed execution orders, final formats, sealed dynamic exports, sealed side effects, and the resolved-export draft through `BindImportsPass`. That pass owns symbols and dependencies, borrows the other facts, preserves the existing serial immediate commit order, and returns a one-call envelope that the driver destructures at once. Its private external binding groups are committed to facade symbols before it returns. `FinalizeResolvedExportsPass` then consumes the draft after the last production `SymbolRefDb::link` call in Link and pairs every raw map with a sorted canonical non-ambiguous map. `ComputeCjsRoutingPass` returns only sparse importers with real routes. `ResolveMemberExpressionsPass` owns that draft plus the JSON guard, constants, and dependencies; it drops named routes and the JSON guard, returns checked normal-only member resolutions and namespace-only `CjsRoutingFinal`, and leaves the final export artifact available for entry-root collection, synthetic statement creation, and H. It intentionally runs before synthetic export statements because its JSON mutation and escape analysis must observe the original statement graph.

After member resolution, `CollectEntryExportRootsPass` borrows the entry plan, final wrappers, final resolved exports, dynamic-import usage, and preserve-signature settings and returns only the sparse ordered roots needed by inclusion and dependency finalization. Each per-entry vector already uses the legacy destination tuple shape, so its final consumer can move the allocation without rebuilding its elements. `CreateSyntheticExportStatementsPass` then owns `IndexStmtInfos`, borrows final formats, resolved exports, missing-export shims, and external-star records, and returns the updated table. `ReferenceNeededSymbolsPass` owns that statement table together with `SymbolRefDb`; its physical per-module work runs in parallel over disjoint owned slots, returns the two entity tables, seals statement-to-runtime-helper requirements for inclusion, and defers only `CallRuntimeRequire` import-record metadata as a narrow ordered patch artifact. `CrossModuleOptimizationPass` follows N, owns only the statement table and global-constant draft, borrows the entry plan, sorted modules, member resolutions, symbols, ASTs, modules, and options, seals unreachable dynamic imports, and finalizes constants.

`TreeShakePass` then consumes `EntryPlanDraft` and borrows only typed facts. Its first phase owns the entry fixpoint, namespace reasons, JSON replacement patches, dead-dynamic-import patches, normalized runtime-helper draft, and used-symbol sets. Runtime-helper inclusion runs in a fresh second-phase context that shares only statement/module inclusion bits and the two used-symbol sets; its namespace reasons, JSON patches, bailout state, body-demand state, pending queue, and change flag are discarded. `FinalizeModuleDependenciesPass` consumes the dependency and runtime-helper drafts, analyzes every physical module against the same immutable pre-commit snapshots, and commits collected results serially in ascending `ModuleIdx` order. This preserves both dependency insertion orders and prevents same-pass transitive `ToEsm` inheritance.

The shared pipeline context drains after dependency finalization. The output adapter immediately destructures its final typed artifacts, validates every dense count and normal/external slot shape before writing output, applies JSON replacement before `DeadDynamicImport` insertion and then `CallRuntimeRequire` patches, and feeds the owned dense slots through one physical-order `izip!` iterator. Its single map closure mutates each module and returns the matching `LinkingMetadata` directly into an `IndexVec`, avoiding separate capacity bookkeeping and pushes while preserving the legacy order. Compact TLA and dynamic-export bitsets and CJS routes remain sparse projections after that assembly; entry-root vectors preserve `(symbol_ref, came_from_commonjs)` order and move directly into their final metadata fields without allocation or element conversion. Executable source guards keep the facade at two fields, require its complete entry destructure, forbid an exact `LinkStage` path or a replacement struct in the adapter module, and pin the adapter's explicit final-field and helper signatures. No pass receives `LinkStage`, metadata, the shared inclusion-core context, or a replacement aggregate.

Each artifact stays typed through its actual final consumer: formats, wrappers, resolved exports, CJS routing, included CommonJS symbols, constants, and normal export chains through H; side effects, member resolutions, entry roots, dependency drafts, and runtime requirements through G; and execution order, TLA, dynamic exports, shims, external stars, sorted modules, merge maps, inclusion results, retained entries, used-symbol continuations, and deferred patches through the output adapter. `StatementRuntimeRequirements` and `UnreachableDynamicImports` are dropped immediately after H, while G consumes both dependency drafts. The lazy pass also emits two purpose-specific sparse artifacts rather than widening a carrier: `NonSplittableJsonDefaults` moves into and dies within `ResolveMemberExpressionsPass`, while `LazyJsonExportInitializers` survives only until Generate materializes retained snapshot bindings immediately after their payload statements.

Splitting import binding into analysis and commit is deliberately rejected for this production cutover, not left as unfinished migration work. Recursive matching observes earlier shim and facade allocation, namespace aliases, canonical symbol links, dependency insertion, and diagnostic order. A pure plan would have to simulate that evolving state or retain a second ordered transcript of all effects, adding another binding-sized representation without improving the current narrow pass boundary. Such a rewrite requires separate semantic and profile evidence.

Artifacts choose their result channel according to lifetime:

- A link-local fact that later passes only read uses `OutputRead`, becomes `Sealed<T>`, and is dropped after its last link consumer.
- A final domain value that must be moved into `LinkStageOutput` or legacy `LinkingMetadata` uses `OutputOwned`. The driver may lend `&T` to later passes, then the output adapter consumes the value by ownership.
- Core mutable working sets such as modules, symbols, ASTs, and statements remain owned and move between their actual mutators before the adapter consumes them.

There is intentionally no general `Sealed::into_inner`, adapter-only unwrap, or clone of a large artifact. If a value must be re-owned at the legacy boundary, it was modeled in the wrong channel if it became `Sealed`; keep it as a domain-specific owned final type with a narrow API. A compact sealed fact may instead be projected into a legacy field at the adapter and then dropped. This compatibility escape is explicit because Generate requires the old mutable representation; making that boundary immutable is a separate project.

Fixed-size conversions are also rejected for this cutover. The dense artifacts already hide mutation behind private `IndexVec` or bitset APIs and validate their exact physical-module layout. Drafts that need mutation or owned parallel iteration already expose only that narrow capability; final facts expose read-only queries, then the adapter either projects selected fields or moves an existing owned container. Reboxing them would establish no additional identity invariant and could add repacking or allocation at the adapter. Explicit last-use scopes and one-time metadata construction provide the intended lifecycle control without speculative representation churn.

## Enforcement boundary

The design uses compiler checks where Rust can express the rule and records the remaining rules explicitly.

### Compiler-enforced and test-pinned

- Safe external code cannot construct `RunToken`, inspect `RawPassOutput`, or construct or unwrap `Sealed` because their fields and constructors are private.
- A token is tied invariantly to one invocation lifetime and one pass type; the `'static` associated output and error types block the supported leak channels.
- `InputRead<'a>: Copy` rejects `&mut T` directly and inside an ordinary copied manifest.
- `Pass: Copy + 'static` rejects non-static pass values, while a const assertion inside `run_pass` rejects a non-zero-sized concrete pass when code is generated.
- `Sealed<T>` exposes no `DerefMut`, consuming unwrap, or public field.
- `PassCtx` exposes writes but no reads or drains.
- Driver order, moves, borrows, and disjoint owned inputs use ordinary Rust name and ownership checks.
- The production link pass subtree forbids unsafe code. Its non-vacuous AST inventory requires every `XxxPass` to be one `pub(super)` non-generic unit struct with one direct unqualified `Pass` implementation, rejects aliases and hidden local declarations, limits derives and production macros to a closed exact-path allowlist, recursively inspects allowed macro arguments, and rejects the broad link carriers in pass code. Binding's preserved tracing and missing-export formatting macros are admitted only at their fully qualified paths in `bind_imports.rs`; neighboring paths and bare names remain rejected. Conditional production attributes are limited to the exact private WASM `IteratorExt as _` compatibility import and the exact private native `IndexedParallelIterator` imports used by M and N; the inventory parses both complete items and rejects other targets, paths, visibility, aliases, or absolute-path variants.

The compile-fail suite pins these claims in the same consuming-crate privacy layout used by future link passes. Its passing case is load-bearing: it makes trybuild perform a code-generating build so the inline non-zero-size assertion is evaluated. `cargo check` and rust-analyzer's usual check-on-save are not enough to enforce that assertion.

### Source-test and review-held

- The source inventory is a repository test rather than a language rule; new syntax forms and deliberate hidden dependency channels still require review and corresponding inventory coverage.
- Pipeline facts expose no interior mutability through `Cell`, `RefCell`, atomics, locks, or safe shared-mutation methods. `InputRead: Copy` rejects `&mut`; it cannot reject `&Cell<T>` or a shared reference to another interior-mutable API.
- Passes do not use global mutable state or other hidden channels. The type contract governs declared parameters, not global behavior.
- Pass slot manifests never name the driver-owned `PassPipelineCtx`; only the harness receives it and derives the temporary `PassCtx`.
- Input and output artifacts remain narrow, named, and purpose-specific. The compiler cannot distinguish a useful domain table from a renamed state bag.
- A pass does not duplicate a designated read-side fact into `OutputOwned` or `Error`, and a sealed artifact does not expose `Clone` when that would create a meaningful mutable copy. Rust checks the declared types and access paths, not the semantic identity of two values.
- Domain finalization uses draft/final types and compact representations where they express real invariants. `Sealed<T>` only controls access through the wrapper.
- Whole-pass concurrency requires semantic independence, deterministic collection and commit order, and a measured benefit. Disjoint Rust borrows prove memory safety, not equivalence of side effects or diagnostics.

## Parallelism

The signatures make candidates visible: two passes can overlap when their owned inputs are disjoint and neither consumes the other's output. The borrow checker rejects shared ownership mistakes, but the driver still gives each branch a separate `PassPipelineCtx` and merges diagnostics in declared order.

Pass-internal data parallelism remains the default for independent per-module work. `CollectResolvedExportsPass`, for example, runs physical roots in parallel but keeps each path-dependent DFS serial. `FinalizeModuleDependenciesPass` performs immutable per-module analysis in parallel and then commits in physical module order, so parallel execution cannot expose a same-pass write to another module's analysis. Driver-level `rayon::join` is appropriate only after state separation makes the dependency graph honest and repeated link-only measurements show a benefit. Plugin calls, I/O, globals, shared diagnostics, and order-dependent commits do not belong in concurrent branches.

The contract is synchronous because link work is CPU-bound. Awaiting plugin hooks and user callbacks stay at driver boundaries. An `AsyncPass` variant is deferred until a selected flow genuinely needs to suspend inside a pass.

## Trait reassessment

The production Link driver now exercises the trait across twenty-four real passes, so the reassessment is based on the complete pipeline rather than a small trial. The trait is retained: the branded token, `InputRead: Copy`, owned mutation slots, unconditional sealed read output, typed infallibility, tracing span, and diagnostic provenance form one enforced contract. The GAT and slot ceremony is visible, but removing the trait would replace those checks with twenty-four hand-maintained conventions. The scope remains deliberately narrow: synchronous top-level passes use the trait, while visitors, helpers, recursion frames, and per-module loops remain ordinary functions.

## Rejected alternatives

- Calling `Pass::run` directly by convention: this bypasses sealing, tracing, and diagnostic provenance; the branded token closes that path for safe reachable code.
- A field-private but unbranded `RunToken<P>`: a hostile pass can choose the token as its error or output type, leak it, and reuse it.
- Hard-coding `BuildResult`: this incorrectly makes an infallible stage look fallible and encourages `?`, panic, or unwrap at an unchanged boundary.
- Making the trait or method private: passes in another crate could not implement the shared harness trait.
- `Arc<T>` as a seal: a unique holder can recover mutability with `Arc::get_mut` or ownership with `Arc::try_unwrap`; use `Arc<Sealed<T>>` when sharing a sealed fact is actually needed.
- A general unwrap for `Sealed<T>`: it would turn the mutation barrier into a convention and let any driver re-own a read-side fact.
- Dynamic pass collections: they erase each pass's distinct input and output signature.
- Parallelizing every independent-looking pass: signature independence is only the first gate; profiling and deterministic behavior decide whether concurrency ships.

## Related

- [implementation.md](./implementation.md) — the implemented harness, its tests, and link's adoption boundary
- `../linking/` — current link behavior that the migration must preserve
