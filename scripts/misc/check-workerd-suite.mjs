// End-to-end suite for `@rolldown/browser/workerd` running inside REAL workerd.
//
// Why this exists: every other workerd check in this repo either runs the
// workerd entry under Node (`packages/rolldown/tests/workerd-*.test.ts`,
// `scripts/misc/check-workerd-memory.mjs`) or exercises real workerd only as a
// packaging smoke with a two-module build
// (`scripts/misc/check-workerd-packed-consumer.mjs`). Nothing asserted that a
// realistic multi-module build, the error surface, the lifecycle and admission
// contracts, or the per-rebuild memory slope actually behave inside workerd.
// This script does, through Miniflare, which embeds the same workerd binary a
// Cloudflare deployment runs.
//
// Transport choice: an explicit Miniflare `modules` list pointed straight at
// `packages/browser/dist`, rather than `wrangler deploy --dry-run` + a bundle.
//   * `check-workerd-packed-consumer.mjs` already proves the wrangler/packaging
//     path, so re-bundling here would buy no new signal for ~20s per run.
//   * The workerd entry reaches its async-context provider through a GUARDED
//     dynamic `import('node:async_hooks')`. Miniflare's automatic module
//     collection statically scans the entry and rejects that specifier at
//     config time; an explicit module list lets workerd resolve it at runtime
//     exactly as a real deployment does.
// The tradeoff is that this script tests the dist artifacts directly rather
// than a wrangler-bundled copy of them -- which is precisely the split of
// responsibilities with the packed-consumer check.
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { Miniflare } from 'miniflare';

const repoRoot = fileURLToPath(new URL('../../', import.meta.url));
const MiB = 1024 * 1024;

const args = new Map(
  process.argv
    .slice(2)
    .map((arg) => arg.split('=', 2))
    .filter((entry) => entry.length === 2),
);
const flags = new Set(process.argv.slice(2).filter((arg) => !arg.includes('=')));

const distDir = path.resolve(repoRoot, args.get('--dist') ?? 'packages/browser/dist');
const rounds = Number(args.get('--rounds') ?? 12);
const slopeModules = Number(args.get('--modules') ?? 300);
// Rounds to discard before measuring: the first rebuilds on a fresh instance
// also pay one-time allocations that are not a per-rebuild leak.
const warmupRounds = Number(args.get('--warmup') ?? 2);
// `--measure` reports the slopes without enforcing the budgets. Use it to
// re-derive the numbers below after an intentional memory change.
const measureOnly = flags.has('--measure');

// ---------------------------------------------------------------------------
// MEMORY BUDGETS -- MiB of Wasm linear memory retained per identical rebuild
// on ONE reusable instance.
//
// !!! THESE BUDGETS ENCODE A KNOWN, UNFIXED LEAK -- TIGHTEN THEM WHEN IT LANDS.
//
// `BindingRenderedChunk` has no `dropInner()`, so the native payload behind
// every rendered chunk handed to a JavaScript hook stays alive for the whole
// lifetime of the instance. `freeOutputs()` can release `BindingOutputs`
// chunks/assets; nothing can release a rendered chunk. The numbers below are
// therefore NOT "flat" -- they are the CURRENT measured slopes plus headroom.
// Their job today is to catch a REGRESSION (the leak growing, or a currently
// free path starting to leak) and to fail loudly when that happens.
//
// >>> WHEN `BindingRenderedChunk::dropInner()` LANDS: re-run this script with
// >>> `--measure` and drop every budget to just above the new numbers. If they
// >>> come back near zero, collapse this table to one flat budget and delete
// >>> this block. Leaving these budgets in place would silently accept the
// >>> leak forever, which is exactly the failure mode this suite exists to
// >>> prevent.
//
// Measured 2026-07-28 on the DEBUG wasm that BOTH CI lanes build (~29.8 MiB
// `rolldown-binding.wasm32-wasip1.wasm`), 300-module graph, 12 rounds, 2
// warmup rounds; three consecutive runs agreed to within 0.014 MiB/rebuild
// (two 64 KiB Wasm pages). Budgets carry ~25% headroom over the measured
// value to absorb that quantisation plus toolchain differences between this
// machine and the CI builder. That headroom is far below the signal each
// variant is guarding: a generateBundle release regression would push
// `generatebundle` to ~0.53, and any doubling of the base leak puts
// `nohooks` at ~0.57 -- both well over budget.
const MEMORY_BUDGETS = {
  // Baseline: `build()` materialises one rendered chunk for the caller.
  nohooks: { measured: 0.285, budget: 0.38 },
  // A generateBundle hook that READS the chunks it is given. Currently costs
  // NOTHING extra, because the per-invocation bundle copy IS released when the
  // hook returns -- the contract pinned by
  // packages/rolldown/tests/workerd-output-ownership.test.ts. This variant is
  // the regression guard on that release: if it ever starts tracking
  // `renderchunk-nomodules`, the generateBundle copy stopped being freed.
  generatebundle: { measured: 0.285, budget: 0.38 },
  // Receiving a BindingRenderedChunk in renderChunk. THIS is the leak.
  'renderchunk-nomodules': { measured: 0.528, budget: 0.66 },
  // ...plus marshalling one BindingRenderedModule per module for chunk.modules
  // (a further ~0.021 MiB/rebuild on a 300-module graph).
  renderchunk: { measured: 0.549, budget: 0.7 },
};

// ---------------------------------------------------------------------------
// Artifact preflight. A lane that silently skips is worse than no lane, so a
// missing artifact is a hard failure that names the command that produces it.
const workerdEntry = path.join(distDir, 'workerd.browser.mjs');
const wasmBinary = path.join(distDir, 'rolldown-binding.wasm32-wasip1.wasm');
const workerSource = path.join(repoRoot, 'scripts/misc/workerd-suite/worker.js');

for (const [label, file] of [
  ['@rolldown/browser workerd entry', workerdEntry],
  ['single-thread WASI binary', wasmBinary],
  ['workerd suite worker', workerSource],
]) {
  if (!existsSync(file)) {
    throw new Error(
      `Missing ${label}: ${path.relative(repoRoot, file)}\n` +
        'Build it first with `just build-browser` (or, on the WASI lane, ' +
        '`just build-rolldown-wasi-single` followed by ' +
        "`vp run --filter '@rolldown/browser' build-node`).",
    );
  }
}

const workerSourceText = await readFile(workerSource, 'utf8');

/**
 * Run one request against a FRESH workerd isolate.
 *
 * The module names are derived from each path relative to `modulesRoot`, so
 * the worker's `./workerd.browser.mjs` / `./rolldown-binding.wasm32-wasip1.wasm`
 * specifiers resolve to the real dist artifacts. `contents` is supplied only
 * for the worker itself, which lets its source live in this repo's scripts
 * tree instead of being staged into `dist/`.
 */
async function dispatch(route) {
  const miniflare = new Miniflare({
    compatibilityDate: '2026-06-01',
    // `nodejs_als` is what makes AsyncLocalStorage available, which is how the
    // workerd entry propagates async context across plugin hooks.
    compatibilityFlags: ['nodejs_als'],
    modulesRoot: distDir,
    modules: [
      {
        type: 'ESModule',
        path: path.join(distDir, '__rolldown-workerd-suite.js'),
        contents: workerSourceText,
      },
      { type: 'ESModule', path: workerdEntry },
      { type: 'CompiledWasm', path: wasmBinary },
    ],
  });
  try {
    const response = await miniflare.dispatchFetch(`http://localhost${route}`);
    const body = await response.text();
    assert.equal(
      response.status,
      200,
      `worker returned ${response.status}: ${body.slice(0, 2000)}`,
    );
    return JSON.parse(body);
  } finally {
    await miniflare.dispose();
  }
}

const started = Date.now();
const elapsed = () => `${((Date.now() - started) / 1000).toFixed(1)}s`;

console.log(`workerd suite: dist=${path.relative(repoRoot, distDir)}`);

// ===========================================================================
// PART 1 -- behaviour: build, errors, lifecycle, concurrency, capabilities.
// ===========================================================================
const behavior = await dispatch('/behavior');
if (behavior.error) throw new Error(`worker failed before reporting: ${behavior.error}`);
assert.equal(behavior.ok, true, `a case threw unexpectedly: ${JSON.stringify(behavior, null, 2)}`);

const { multiModuleBuild, errorSurface, lifecycle, concurrency, capabilities } = behavior.cases;

// --- CASE 1: multi-module build through the high-level API -----------------
assert.equal(multiModuleBuild.chunkCount, 1, 'expected exactly one chunk');
assert.equal(multiModuleBuild.loadedCount, 26, 'load() must have served all 26 fan modules');
assert.equal(
  multiModuleBuild.transformedCount,
  21,
  'transform() must have rewritten 20 leaves + the entry',
);
assert.ok(
  multiModuleBuild.resolvedDistinct >= 26,
  `resolveId() must have resolved every module, saw ${multiModuleBuild.resolvedDistinct}`,
);
assert.ok(
  multiModuleBuild.moduleCount >= 20,
  `chunk must retain a 20+ module graph, saw ${multiModuleBuild.moduleCount}`,
);
assert.equal(multiModuleBuild.leftoverLeafToken, false, 'a __LEAF_FACTOR__ token survived');
assert.equal(multiModuleBuild.leftoverStampToken, false, 'a __STAMP__ token survived');
assert.equal(multiModuleBuild.codeHasAnsi, false, 'generated code must be ANSI-free');
assert.equal(multiModuleBuild.generateBundleCalls, 1, 'generateBundle must run exactly once');
assert.equal(multiModuleBuild.assetCount, 1, 'generateBundle emitFile must add one asset');
assert.deepEqual(
  JSON.parse(multiModuleBuild.manifestSource),
  { entries: [multiModuleBuild.fileName] },
  'the emitted manifest must describe the real bundle',
);
assert.equal(multiModuleBuild.liveDelta, 0, 'build({module}) must dispose its private instance');

// Prove the OUTPUT is correct, not merely well-shaped: execute the chunk that
// workerd generated and check the value it computes. (workerd itself forbids
// dynamic code evaluation, so this half necessarily runs in Node.)
const generated = await import(
  `data:text/javascript;base64,${Buffer.from(multiModuleBuild.code).toString('base64')}`
);
assert.equal(
  generated.total,
  390,
  `workerd-generated chunk computed total=${generated.total}, expected 390`,
);
assert.equal(generated.stamp, '-transformed-by-workerd-suite');
assert.equal(generated.moduleTag, '390-transformed-by-workerd-suite');
console.log(
  `  [1/6] multi-module build       ok  (${multiModuleBuild.moduleCount} modules in chunk, ` +
    `total=${generated.total}) ${elapsed()}`,
);

// --- CASE 2: error surface -------------------------------------------------
const caught = errorSurface.caught;
assert.ok(caught, 'the unresolvable import must reject');
assert.equal(caught.isError, true, 'the rejection must be a real Error');
assert.match(caught.message, /definitely-not-resolvable/);
assert.ok(caught.errorCount >= 1, '`errors` must be populated');
assert.ok(
  caught.errorMessages.some((message) => /definitely-not-resolvable/.test(message)),
  '`errors[].message` must name the unresolved import',
);
assert.equal(caught.hasStack, true, 'the rejection must carry a stack');
// The rendered code frame is inlined into `message` (there is no separate
// `.frame` property on this error). Assert the frame is really there, so the
// ANSI check below is guarding real diagnostic output rather than a bare
// one-line message.
assert.match(caught.message, /UNRESOLVED_IMPORT/);
assert.match(caught.message, /fan:bad-entry\.js:1:19/, 'the code frame must carry a location');
assert.match(
  caught.message,
  /import \{ q \} from '\.\/definitely-not-resolvable\.js';/,
  'the code frame must quote the offending source line',
);
// message / stack / frame -- and the same fields on every entry of `errors` --
// must be free of ANSI escapes: workerd has no TTY and these strings are
// routinely serialised into logs and JSON.
assert.deepEqual(caught.ansiAt, [], `ANSI escapes leaked into: ${caught.ansiAt.join(', ')}`);
assert.equal(errorSurface.reuse.ok, true, 'the same instance must build after a failed build');
assert.equal(errorSurface.reuse.loadedCount, 26);
assert.equal(errorSurface.disposed, true);
assert.equal(errorSurface.liveDelta, 0, 'a failed build must not leak an instance');
console.log(
  `  [2/6] error surface            ok  (${caught.errorCount} error(s), code frame ` +
    `inlined in message, ${caught.frameCount} separate .frame, no ANSI) ${elapsed()}`,
);

// --- CASE 3: lifecycle contracts -------------------------------------------
assert.equal(lifecycle.closedBeforeClose, false);
assert.equal(lifecycle.generateOk, true, 'an open bundle must generate');
assert.equal(lifecycle.disposeWhileOpen.ok, false, 'dispose() with an open bundle must be refused');
assert.match(
  lifecycle.disposeWhileOpen.error.message,
  /Cannot dispose this workerd Rolldown instance/,
);
assert.deepEqual(lifecycle.disposeWhileOpen.error.ansiAt, []);
assert.equal(lifecycle.closedAfterClose, true, 'close() must mark the bundle closed');
assert.equal(lifecycle.generateAfterClose.ok, false, 'generate() after close() must fail');
assert.equal(lifecycle.generateAfterClose.error.isError, true);
assert.deepEqual(lifecycle.generateAfterClose.error.ansiAt, []);
assert.equal(lifecycle.otherInstanceAfterClose.ok, true, 'close() must release the slot');
assert.equal(lifecycle.disposeAfterClose.ok, true, 'dispose() must succeed once the bundle closed');
assert.equal(lifecycle.instanceADisposed, true);
assert.equal(lifecycle.instanceBDisposed, true);
assert.equal(lifecycle.liveDelta, 0, 'the lifecycle case must leak no instance');
console.log(`  [3/6] lifecycle contracts      ok  ${elapsed()}`);

// --- CASE 4: concurrency and admission -------------------------------------
assert.equal(
  concurrency.concurrentSameInstance.first.ok,
  true,
  `first concurrent build failed: ${JSON.stringify(concurrency.concurrentSameInstance.first)}`,
);
assert.equal(
  concurrency.concurrentSameInstance.second.ok,
  true,
  `second concurrent build failed: ${JSON.stringify(concurrency.concurrentSameInstance.second)}`,
);
assert.equal(
  concurrency.secondInstanceWhileActive.ok,
  false,
  'a build on a second instance must be refused while another is active',
);
assert.match(
  concurrency.secondInstanceWhileActive.error.message,
  /Another workerd Rolldown instance is currently active in this module/,
);
assert.equal(concurrency.slowBuild.ok, true, 'the parked build must still complete');
assert.equal(
  concurrency.secondInstanceAfterRelease.ok,
  true,
  'the second instance must be admitted once the slot is free',
);
assert.equal(concurrency.liveDelta, 0, 'the concurrency case must leak no instance');
console.log(`  [4/6] concurrency + admission  ok  ${elapsed()}`);

// --- CASE 5: capabilities as reported INSIDE workerd -----------------------
assert.equal(capabilities.target, 'wasi');
assert.equal(capabilities.flavor, 'CurrentThread');
assert.equal(capabilities.threads, false, 'workerd must never report thread support');
assert.equal(capabilities.wasi, true);
assert.equal(capabilities.watchSupported, false, 'watch is unsupported in workerd');
assert.equal(capabilities.devSupported, false, 'dev is unsupported in workerd');
assert.equal(capabilities.blockOnJsThreadSafe, false);
assert.equal(capabilities.watchExported, false, 'the workerd entry must not export watch()');
assert.deepEqual(
  capabilities.entryExports,
  [
    'WORKERD_WASM_MEMORY',
    'build',
    'configureAsyncContext',
    'createInstance',
    'createWorkerdBundle',
    'freeOutputs',
    'getWorkerdRuntimeStats',
    'instantiate',
  ],
  'the workerd entry export surface changed',
);
assert.ok(capabilities.memoryBytes > 0);
assert.equal(behavior.finalLiveInstances, behavior.baselineLiveInstances, 'an instance leaked');
console.log(
  `  [5/6] capabilities             ok  (target=${capabilities.target}, ` +
    `flavor=${capabilities.flavor}, threads=${capabilities.threads}) ${elapsed()}`,
);

// ===========================================================================
// PART 2 -- memory slope with plugin hooks.
// ===========================================================================
/** Post-warmup MiB of Wasm linear memory retained per identical rebuild. */
function slopeMiBPerRound(memPerRound) {
  const post = memPerRound.slice(warmupRounds);
  assert.ok(post.length >= 2, `need at least ${warmupRounds + 2} rounds to measure a slope`);
  return (post.at(-1) - post[0]) / (post.length - 1) / MiB;
}

const slopes = {};
const failures = [];
for (const variant of Object.keys(MEMORY_BUDGETS)) {
  const report = await dispatch(
    `/memory?variant=${variant}&rounds=${rounds}&modules=${slopeModules}`,
  );
  if (report.error) throw new Error(`memory variant ${variant} failed: ${report.error}`);
  assert.equal(report.memPerRound.length, rounds, `variant ${variant} did not run every round`);
  if (variant !== 'nohooks') {
    assert.equal(report.hookCalls, rounds, `variant ${variant} hook did not run every round`);
    assert.ok(report.modulesSeen > 0, `variant ${variant} hook observed nothing`);
  }
  // Identical rebuilds must produce identical output, or the slope is not
  // comparing like with like.
  assert.equal(
    new Set(report.outputBytesPerRound).size,
    1,
    `variant ${variant} produced different output across rounds`,
  );

  const slope = slopeMiBPerRound(report.memPerRound);
  slopes[variant] = slope;
  const { budget } = MEMORY_BUDGETS[variant];
  const verdict =
    measureOnly || budget === null ? 'measured' : slope <= budget ? 'ok' : 'OVER BUDGET';
  console.log(
    `  [6/6] memory ${variant.padEnd(15)} ${verdict.padEnd(11)} ` +
      `${slope.toFixed(3)} MiB/rebuild` +
      (budget === null ? '' : ` (budget ${budget.toFixed(3)})`) +
      `  [${report.memFirstMiB} -> ${report.memLastMiB} MiB over ${rounds} rounds]`,
  );
  if (!measureOnly && budget !== null && slope > budget) {
    failures.push(
      `memory variant '${variant}' retained ${slope.toFixed(3)} MiB/rebuild, ` +
        `budget is ${budget.toFixed(3)} MiB/rebuild. The known BindingRenderedChunk ` +
        'leak GREW, or a new one was introduced. Investigate before raising this budget.',
    );
  }
}

if (measureOnly) {
  console.log(`\nMEASURED SLOPES (--measure, budgets not enforced):`);
  console.log(JSON.stringify(slopes, null, 2));
} else if (failures.length > 0) {
  throw new Error(`\n${failures.join('\n')}`);
}

console.log(
  `\nOK: @rolldown/browser/workerd builds, fails, disposes, admits and retains ` +
    `memory as expected inside real workerd (${elapsed()})`,
);
