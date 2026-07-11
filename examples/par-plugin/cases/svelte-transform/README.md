# Direct-Rolldown Svelte Transform Fixture

This fixture compares one ordinary plugin instance with the current ParallelPlugin worker pool around the same Svelte 5.56.4 compiler kernel. It runs Rolldown directly and does not involve Vite, preprocessing, watch, rebuild, or HMR.

## Pinned corpus

The source corpus is `huntabyte/shadcn-svelte@efcf8a4ef2c6a3a21ee2fd4db905519f8d4c8e63`. The selection starts with tracked `docs/src/lib/registry/**/*.svelte` files and excludes 26 files whose UTF-8 source contains an ASCII case-insensitive `<svg` substring. That intentionally excludes the one `SVGAttributes<SVGSVGElement>` occurrence as well as SVG tags, matching the pinned research set.

The committed manifest records all 1,340 paths and content hashes. It verifies 64,392 lines, 1,946,145 bytes, 1,314 TypeScript-script components, 616 rune-using components, 1,322 unique contents, and aggregate SHA-256 `ea584b2189062d5986cb4c15f344bcb42cbee8b7089277ee95d5d7ab9f49b8e8`. `upstream-LICENSE.md` is the exact upstream MIT license.

Prepare an offline local snapshot from the pinned checkout before benchmarking:

```sh
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prepare-corpus.mjs --source /path/to/shadcn-svelte
```

Preparation rescans and hashes every source file, compares every entry with `corpus-manifest.json`, verifies the upstream license, and atomically writes the ignored `.corpus/` snapshot. Matrix runs rehash the prepared snapshot before starting any measured child. Case prefixes use `sha256(corpusAggregate + NUL + path)` rather than lexicographic order, so small cases are stable without concentrating one directory.

## Compiler boundary

Both adapters call the same `createSvelteTransformPlugin` kernel with `generate: 'client'`, `dev: false`, `css: 'injected'`, and `discloseVersion: false`. The ordinary adapter imports the compiler in the main process. The parallel marker imports only its lightweight marker on the main process; each worker imports Svelte and creates an independent compiler instance.

Every selected component is re-exported by one generated entry so its compiled module body remains in the output. Dependencies emitted or retained by a compiled component are externalized, so this measures Svelte compilation plus Rolldown's parsing, source-map chaining, module processing, and output generation for the compiled result. It is not a full shadcn-svelte application build. The corpus has no `<style>` tags, and the fixture deliberately omits preprocessing, virtual CSS, function-valued options, cross-hook metadata, and application dependency resolution.

Each run generates a bundle source map and requires ordinary and worker variants to have identical raw and normalized code hashes and map hashes. Instrumented matrices additionally validate handler calls, exact input bytes, per-worker distribution, maximum JavaScript handler concurrency, worker factories, Rust wrapper results, filter misses, permits, and lifecycle metrics. Instrumentation is explanatory only; wall-time claims use `instrumentation: false`.

## Commands

Build the optimized native binding and use only the pinned Node.js binary:

```sh
mise exec node@24.18.0 -- just build-rolldown-release
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./smoke-matrix.json ./.results/smoke.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./full-smoke-matrix.json ./.results/full-smoke.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-matrix.json ./.results/wall.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./summarize-matrix.mjs ./.results/wall.json ./.results/wall-summary.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./isolation-matrix.json ./.results/isolation.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-semantics.mjs ./.results/semantics.json
```

Every measured sample is a fresh Node.js process and a fresh worker pool. `totalElapsedMs` starts after process startup, runner imports, and option parsing, immediately before the plugin adapter is imported; it ends after `build.close()`, before output hashing. It includes the ordinary compiler import or parallel marker import, plugin setup, `rolldown()`, `generate()`, and `close()`. The external peak RSS wraps the whole child and therefore also covers output hashing. The runner performs one discarded fresh-process warmup per variant, rotates measured variant order, records CPU and peak RSS, and pins the exact Node binary, Rolldown commit, worktree status, native binding byte hash, host, matrix, corpus, selection summaries, and all raw samples. `bindingProfile: "release"` records the required build command's intended profile; the artifact hash pins the binary but cannot independently prove its Cargo profile.

The semantics probe separately records Svelte warning delivery and compile-error structure for ordinary and worker variants. It does not hide a mismatch: `sameLogs` and `sameStructuredError` are explicit report fields because the current worker-side plugin context may not preserve coordinator logging and diagnostic fields.
