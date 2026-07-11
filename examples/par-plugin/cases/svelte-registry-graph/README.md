# Direct-Rolldown Svelte Registry UI Graph

This fixture is a graph-preserving docs registry UI subgraph with package boundaries external. It is not a full shadcn-svelte monorepo, docs application, SvelteKit, or Vite build.

The 56 real `docs/src/lib/registry/ui/*/index.ts` barrels are passed directly to Rolldown as separate entries. There is no generated aggregator. Rolldown follows every project-local relative, absolute, and `$lib` edge reached from those entries. The coordinator resolver only implements the project's `$lib` alias and its NodeNext source convention: `.js` resolves to `.ts`, including `.svelte.js` resolving to `.svelte.ts`. The runner rejects any externalized relative, absolute, or `$lib` ID.

Bare boundaries are recorded separately: SvelteKit `$app/*` virtual modules, `shadcn-svelte/*` workspace package exports, Svelte runtime imports, and third-party packages. Following `shadcn-svelte/*` would require building and resolving the CLI workspace package's exports and adds no Svelte component transforms, so it remains an explicit package boundary rather than being described as third-party.

The prepared source snapshot contains all 2,607 tracked files under `docs/src` at `huntabyte/shadcn-svelte@efcf8a4ef2c6a3a21ee2fd4db905519f8d4c8e63`. The committed manifest pins 3,535,740 bytes, all 56 barrels, and source aggregate SHA-256 `d7e6608eee8465062fae46ab0343837cdcee39838fadb0106ae24755030c3e4c`. Preparation verifies the upstream commit, license, aggregate, and every copied file.

The shared ordinary/parallel kernel calls Svelte 5.56.4 `compile` for `.svelte` and `compileModule` for reached `.svelte.ts` or `.svelte.js` modules. Svelte's `compileModule` does not parse TypeScript syntax directly, so the same kernel imports full TypeScript and runs `transpileModule` on the four reached rune modules before compilation, passing the TypeScript source map into Svelte as an input map. TypeScript import, transpilation, source-map composition, and per-worker memory are adaptation costs and are included in every result. Handler time is not described as pure Svelte compilation.

Compiler filenames are corpus-relative. No compiler `root` option is used. Hooks return only code and source maps; they do not return `moduleType`.

`expected-graph.json` is generated from an instrumented ordinary build and then acts as a hard gate. It pins every reached local module, transform calls and bytes, resolver counts, external boundaries, logs, output chunks/assets/exports/bytes, null-map chunks, and code/map hashes. Every measured ordinary or parallel process must reproduce it exactly.

## Reproduction

```sh
git clone https://github.com/huntabyte/shadcn-svelte.git /tmp/shadcn-svelte
git -C /tmp/shadcn-svelte checkout --detach efcf8a4ef2c6a3a21ee2fd4db905519f8d4c8e63
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prepare-corpus.mjs --source /tmp/shadcn-svelte

mise exec node@24.18.0 -- just build-rolldown-release
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prove-ordinary.mjs ./.results/ordinary-proof.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./smoke-matrix.json ./.results/smoke.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-matrix.json ./.results/wall.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./summarize-matrix.mjs ./.results/wall.json ./.results/wall-summary.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./instrumented-matrix.json ./.results/instrumented.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./summarize-matrix.mjs ./.results/instrumented.json ./.results/instrumented-summary.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./isolation-matrix.json ./.results/isolation.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./summarize-matrix.mjs ./.results/isolation.json ./.results/isolation-summary.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-semantics.mjs ./.results/semantics.json
```

Every sample is a fresh Node.js process and, for parallel variants, a fresh worker pool. `totalElapsedMs` excludes Node startup, top-level Rolldown import, corpus verification, and result hashing. It includes the resolver and adapter setup, ordinary or per-worker Svelte and TypeScript imports, TypeScript adaptation, compilation, Rolldown build/generate/close, and source-map processing. External peak RSS wraps the complete child and also covers result hashing. Instrumented matrices explain initialization, imports, task concurrency, queue/service time, and payload; speed claims use uninstrumented wall samples only.

Formal runners reject a dirty worktree, hash the exact Node executable and native binding, and record fixture HEAD separately from native binding source commit `54fd0e24112505443044a4bba5c41d1f4d9ba2aa`. Commits after that binding source are restricted to `examples/par-plugin/` and its lockfile entry. This branch inherits the research ParallelPlugin runtime repairs; it is not an experiment against unmodified Rolldown main.
