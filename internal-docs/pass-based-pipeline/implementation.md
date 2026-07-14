# Pass-Based Pipeline — Implementation

> The rationale and enforcement boundary behind this machinery live in [design.md](./design.md).

## Summary

Phase 0 provides a shared synchronous pass harness in `rolldown_utils`, runtime tests for execution and diagnostic behavior, and consuming-crate compile tests for the ownership contract. Link is the first selected adopter, but the repository does not yet contain a real link pass or a changed link driver. This document records the machinery that exists now and the boundary rules that later link phases must follow.

## Component map

| Component | Location | Responsibility |
| --- | --- | --- |
| Public module export | `crates/rolldown_utils/src/lib.rs` | Exposes the shared `pass` module. |
| Harness | `crates/rolldown_utils/src/pass.rs` | Defines `Pass`, the branded execution token, raw result envelope, sealing wrapper, public result aliases, contexts, and execution entries. |
| Runtime tests | `crates/rolldown_utils/tests/pass_harness.rs` | Exercises sealed and owned outputs, typed error propagation, and deterministic branch diagnostic merging. |
| Compile-test driver | `crates/rolldown_utils/tests/pass_harness_ui.rs` | Runs one valid code-generating case and all pass-harness compile-fail cases. |
| Compile fixtures | `crates/rolldown_utils/tests/ui/pass_harness/` | Pins capability, privacy, lifetime, ownership, ordering, sealing, context, and codegen failures from a consuming crate. |
| Future link passes | `crates/rolldown/src/stages/link_stage/passes/` | Reserved for the selected adoption; this subtree and its real pass implementations do not exist yet. |

## Public API and control flow

The implemented shape is:

```text
driver-owned PassPipelineCtx
  └── run_pass / run_infallible_pass
        ├── assert concrete pass size is zero during code generation
        ├── enter the pass tracing span
        ├── create RunBrand and RunToken<'_, P>
        ├── derive temporary write-only PassCtx
        └── P::run(token, cx, read, owned)
              └── token.finish(read_output, owned_output)
                    └── private-field RawPassOutput
        └── harness opens RawPassOutput
              ├── Sealed<OutputRead>
              └── OutputOwned
```

`Pass` has `InputRead<'a>`, `InputOwned`, `OutputRead`, `OutputOwned`, and `Error` associated types. `InputRead<'a>` is `Copy`; all other associated types are `'static`. Its `run` method receives `RunToken<'_, Self>`, where the anonymous token lifetime is bound to that invocation. `PassOutput<P>` names `(Sealed<P::OutputRead>, P::OutputOwned)`, and `PassResult<P>` names the corresponding result with `P::Error`. `run_pass` returns `PassResult<P>` without erasing the concrete associated error. `run_infallible_pass` is restricted to `Error = Infallible`, returns `PassOutput<P>`, and removes the uninhabited error by exhaustive matching.

`RunToken<'run, P>` contains a private mutable borrow of a harness-local `RunBrand`, an invariant lifetime marker, and a pass-type marker. It is neither `Copy` nor `Clone`. Its only public operation, `finish`, consumes it and creates `RawPassOutput`; that envelope's fields are private, so only the harness can open it.

`Sealed<T>` has a private field and only implements shared `Deref`. It has no public constructor, mutable dereference, or consuming unwrap. The harness constructs it unconditionally around `OutputRead`, including `OutputRead = ()`.

## Context ownership and diagnostics

The driver owns `PassPipelineCtx`. A serial pipeline uses one context; concurrent branches use separate contexts and append them to the parent in declared pass order. `append` preserves the order already present in each branch. `into_diagnostics` consumes the context and returns the existing `Diagnostics` type.

Only the harness constructs `PassCtx<'_>`, and each value exists for one invocation. The pass can call `push` or `extend`; it cannot construct, inspect, drain, or retain the underlying pipeline context. The harness stores the emitting pass's concrete type name with each diagnostic. When the pipeline context is consumed, that provenance is emitted to the `rolldown::pass` tracing target before the ordinary diagnostic is returned.

The context is a sink, not a dependency container. Pipeline data must remain in the declared read and owned slots.

## Compile and runtime coverage

`pass_harness.rs` contains three runtime cases:

- An infallible pass returns a sealed read-side fact while its owned output continues into another pass.
- A fallible pass returns its own typed error unchanged.
- Two branch contexts appended in declared order yield diagnostics in that same order. A local recording subscriber also pins the `run_pass` span, `rolldown::pass` event target, and concrete pass provenance for each merged diagnostic.

`pass_harness_ui.rs` first compiles `pass_valid.rs`, then compiles every `fail_*.rs` fixture expecting failure. The passing case is required because trybuild otherwise may use a check-only path; the non-zero-sized pass assertion lives in an inline const inside `run_pass` and must be exercised by code generation.

The fixtures pin these boundaries:

- External code cannot construct `RunToken`, `RawPassOutput`, or `Sealed`, inspect the raw or sealed private fields, unwrap a sealed value, obtain mutable access through it, or read `PassCtx`.
- A caller cannot invoke `Pass::run` without a token, use a token for the wrong pass, or leak the branded token through the associated error, either output, a panic payload, or a spawned thread.
- `InputRead` cannot contain a direct or nested `&mut`, and one owned input cannot be moved into sibling branches.
- Wrong driver order and a literal `()` pattern for `Sealed<()>` fail.
- A non-zero-sized pass fails in the code-generating test.

These tests do not prove the absence of interior mutability, globals, broad artifacts, inappropriate `Clone` access, duplicate values placed in a different output channel, or semantic ordering dependencies. They also do not prevent a pass from naming `PassPipelineCtx` in one of its own slots. Those remain source-test and review responsibilities described in `design.md`.

## Link adoption boundary

The selected adoption is the internals created by `LinkStage::new` and driven by `LinkStage::link`. Its external interface remains unchanged: input is `NormalizedScanStageOutput` plus `SharedOptions`, and `.link()` remains infallible and returns `(LinkStageOutput, IndexEcmaAst, UsedSymbolRefsBuilder)`. Code outside link does not need to understand the pass migration.

`LinkStage` may remain as a one-shot facade that holds only the untouched scan output and options until `.link()` consumes it. It must not remain a persistent cross-pass carrier for derived state. No pass may accept `LinkStage`, `LinkStageOutput`, `LinkingMetadata`, `LinkingMetadataVec`, or a renamed equivalent.

The future driver has three parts:

```text
NormalizedScanStageOutput + SharedOptions
  └── input adapter: move fields, initialize empty values, seed diagnostics
        └── typed link driver: narrow artifacts and owned entity working sets
              └── legacy output adapter: consume final values once
                    └── existing mutable link output tuple
```

Neither adapter is a pass. The input adapter performs no traversal, sorting, aggregation, or link analysis. Passthrough fields remain ordinary driver locals rather than entering the pass DAG. The output adapter is the only place that reconstructs `LinkStageOutput` and legacy `LinkingMetadataVec`.

### Legacy output ownership policy

The output adapter must construct the mutable legacy boundary without weakening `Sealed<T>`:

| Value kind | Pass channel | Driver behavior | Final action |
| --- | --- | --- | --- |
| Link-local fact read by later passes | `OutputRead` | Keep `Sealed<T>` and lend `&T` through dereference. | Drop after its last link consumer, or project a compact fact into one legacy field at the adapter. |
| Domain-final value required by Generate | `OutputOwned` | Keep ownership in a domain-specific final type, lend `&T` to readers, and do not expose general mutation. | Move it once into the legacy output adapter. |
| Mutable entity table or draft | `OutputOwned` | Move it only into passes that mutate or consume it; unrelated passes receive narrow shared reads. | Move it once into the legacy output tuple or drop it at its declared last use. |

There is no `Sealed::into_inner`, no adapter-only general unwrap, and no clone of a large table or map merely to cross the boundary. A value that must be moved into the legacy output stays on the owned channel. Its final domain type may restrict mutators with module privacy, but it is not described as mutation-sealed because Generate eventually receives a mutable legacy representation.

If a compact link-local sealed fact also has a legacy field, the adapter may copy or project that narrow fact and then drop the sealed artifact. This is the temporary policy for facts such as TLA reachability during migration. A projection must name its removal phase; it must not become a second persistent representation or a way to pass legacy metadata back into the DAG.

## Pass module conventions for link

When the first real link pass lands, the link passes subtree must declare `#![forbid(unsafe_code)]`. Each pass token is a non-generic unit struct named `XxxPass`; runtime configuration belongs in its slot manifest. The harness's code-generation size assertion remains a backstop, not a source-shape checker.

The source inventory check should land with that first real pass so it is non-vacuous. It must reject generic, tuple, and braced pass-token declarations and make the inventory of link passes mechanically complete. Until then, the shared harness is the only implemented pass code and this source convention remains an adoption requirement rather than a claimed compiler guarantee.

## Current phase status

- Phase 0 machinery exists in `rolldown_utils`: the branded token, private raw envelope, typed error channel, infallible entry, contexts, sealing wrapper, runtime tests, and compile fixtures are present.
- No production link code calls the harness yet. There is no `ComputeTlaPass`, link passes subtree, typed link driver, input adapter, or output adapter in the current tree.
- Behavior and performance baselines, the first TLA vertical slice, artifact extraction, carrier removal, and measured whole-pass concurrency are later phases. This document must be updated as each becomes real; future artifacts and pass names are plans, not current implementation.

## Related

- [design.md](./design.md) — contract rationale, enforcement boundary, and rejected alternatives
- `../linking/` — current link implementation notes and behavior constraints
