# Runtime Initialization Attribution

This direct Node.js control separates generic `Worker` creation from native-binding import and the complete Rolldown package import. It is an attribution harness, not a build benchmark.

Each fresh case chooses `empty`, `binding`, or `package`, creates one through eight workers concurrently, and preloads `none`, `binding`, or `package` in the parent. Binding or package preload matches the normal direct-Rolldown shape: the process-level napi runtime already exists before parallel-plugin workers import the same addon. Binding-only and package-import deltas separate worker environment/module loading from generic Worker creation.

The original source-derived hypothesis was that every Node worker environment reran Rolldown's Rust `module_init`, constructed another 18-thread Tokio runtime, and discarded it because napi retains one process-global runtime. Instrumented worker-one and worker-four direct builds falsified that hypothesis: each process emitted one constructor record, and the retained runtime started 18 threads without a later stop. Inspection of `napi-derive` 3.5.9 confirms that `#[napi_derive::module_init]` expands through `napi::ctor::declarative::ctor!`, a dynamic-library constructor rather than a per-environment registration callback. Worker import cost must therefore be attributed elsewhere; the harness requires zero or one constructor record per process.

The worker records entry and dynamic-import boundaries with the process-wide Node monotonic clock. The parent records constructor, online, ready, termination, process/main/worker CPU, V8 heaps, and whole-process RSS. Whole-process RSS differences are controlled observations, not worker ownership. Residual CPU includes all unmeasured runtime and native work and is never labelled Rolldown Rust CPU.

An optional macOS `/bin/ps -M` thread sampler exists only for diagnostic smoke. It synchronously forks and cannot provide a one-millisecond cadence; enabling it perturbs short initialization and its elapsed values are invalid. Formal attribution disables it. The native module-init callback counters supply the exact retained Tokio thread observation without process polling.

Run an untimed smoke with:

```sh
node examples/par-plugin/cases/runtime-initialization/run-case.mjs '{"mode":"binding","workerCount":4,"parentPreload":"binding","sampleIntervalMs":5,"sampleOsThreads":false}'
```

Formal repeated initialization attribution requires the same restarted quiet-host gate and artifact provenance as the scale protocol. Its elapsed fields must not be mixed into uninstrumented wall curves.

The attribution profile is research commit `8e35a2249b60b65120a44d1d896eeeed19dc703b`, release binding SHA-256 `6b7dfa175754ac57650768a68d7a567c5c0635a1bb47d47c5287914594c9795e`, and distribution SHA-256 `68f57be9a8883a4ca6f28b57a9bac6e16907d8c1d079686ab9921b407b132735` over 17,140,783 bytes. The smoke admits the harness without host timing; the formal matrix runs ten rotated blocks under the frozen host gate.
