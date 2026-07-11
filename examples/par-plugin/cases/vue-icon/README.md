# Direct-Rolldown Vue Transform Case

This research fixture compares ordinary and parallel execution of the same transform-only adapter around `unplugin-vue/rolldown` 7.2.0. It invokes Rolldown directly and does not run Vite.

## Pinned corpus

`prepare-corpus.mjs` checks out `cabinet-fe/icon` commit `9cadad32c72d79424c75e3b6e56798f216bb0b06` into ignored benchmark storage, then verifies the real `packages/vue/src` corpus before any measured child starts.

- Full case: four upstream entries, 166 SFCs, 109,122 bytes, manifest SHA-256 `9ae54c3311168ccd093c9da5a1e977c81654590ce040a5de63c2702ff0f3fedd`.
- Small negative control: the colorful entry, 12 SFCs, 16,932 bytes, manifest SHA-256 `6b8c33346f17113a20a245c684cc38f8c9549db519a9d27809376b505ea4c083`.

The upstream package declares MIT in `packages/vue/package.json` but does not contain a license file at the pinned commit, so this fixture does not vendor its source. Reproduction checks out the public pinned source instead.

## Adapter boundary

The unchanged full ordinary plugin is a correctness reference only. Timed ordinary and parallel variants call the same `createVueTransformPlugin` factory, expose only `buildStart` and `transform`, preserve hook context, use the same declarative `.vue` filter, set production inline-template compilation, and explicitly configure Rolldown `moduleTypes.vue = 'js'`.

The case covers SFC parse, script setup, TypeScript, inline templates, component IDs, code generation, imports, JIT, and compiler errors. It excludes styles, external and custom blocks, child virtual modules, source maps in final output, non-cloneable compiler options, warnings, watch, rebuild, HMR, and Vite lifecycle. The TypeScript tail invokes Vite's synchronous native Oxc transform, so the measured plugin cost is not purely JavaScript.

## Commands

Build the optimized binding and use the pinned Node binary:

```sh
mise exec node@24.18.0 -- just build-rolldown-release
cd examples/par-plugin/cases/vue-icon
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prepare-corpus.mjs
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./smoke-matrix.json /tmp/vue-smoke.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-confirm-matrix.json /tmp/vue-wall.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./instrumented-matrix.json /tmp/vue-instrumented.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./isolation-matrix.json /tmp/vue-isolation.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-failure-matrix.mjs /tmp/vue-errors.json
```

Every performance sample is a fresh Node process and every parallel sample creates a fresh pool. Wall claims use instrumentation-off reports only. The runner rotates variants, compares every output byte and hash, and rejects call, byte, worker, permit, lifecycle, error, or cancellation mismatches. Instrumented matrices explain startup, import, `buildStart`, handler, queue, payload, concurrency, CPU, and RSS; their wall times are not performance claims.
