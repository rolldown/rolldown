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
// Every hook-input box now has an eager-release path on this flavor
// (`dropInner()` on rendered chunks/meta, module infos, plugin contexts and
// normalized options, wired through the hook wrappers behind
// `shouldEagerlyFreeOutputs()`), so the old unbounded retention -- the native
// payload behind every rendered chunk handed to a JavaScript hook staying
// alive for the whole lifetime of the instance -- is gone: `renderchunk*`
// dropped from 0.528/0.549 to the numbers below, and reading `chunk.modules`
// no longer costs anything over not reading it. What remains is the shared
// per-rebuild build/marshalling slope (see the `nohooks` note) plus a small
// (~0.011 on this pin) per-rebuild cost for installing the extra output hook.
// These budgets exist to catch a REGRESSION -- any release path silently
// dropping out -- and the floors below catch further improvements that would
// otherwise leave stale ceilings behind.
//
// Measured 2026-08-09 with the napi c0c2f0c2 pin (napi main + #3423) on CI
// run 31322883081 at --rounds=20 -- attempts 1 and 2 of the Node Validation
// lane repeated every value exactly. This is a SINGLE-DATAPOINT calibration
// for this verify branch only: the c0c2f0c2 binding genuinely retains ~40%
// less per rebuild than the ad23bdeb integration pin (smaller per-instance
// wrap bookkeeping), which tripped the previous floors, and one CI run is
// enough to re-band the verify lane. The publish re-pin MUST redo the
// standard 5-8 measure-run calibration before these numbers are trusted
// anywhere else. The window length remains part of the number (rebuild
// growth is CONVEX -- dlmalloc arena stepping retains more per round in the
// tail), so these values are only valid at --rounds=20 -- the count ci.yml
// runs. Budgets carry ~25% headroom over the measured value to absorb page
// quantisation plus toolchain differences between machines and the CI
// builder. That headroom is far below the signal each variant is guarding:
// the eager release of the renderChunk argument regressing puts
// `renderchunk-nomodules` back at ~0.55, and any doubling of the base slope
// puts `nohooks` at ~0.39 -- both well over budget.
const MEMORY_BUDGETS = {
  // Baseline -- and NOT "the cost of a bare rebuild". The slope graph exists
  // only inside `slopeGraphPlugin`'s map, so that plugin is structurally
  // mandatory and `caseMemorySlope` installs it for EVERY variant, `nohooks`
  // included. What this number actually contains is one full 305-module build
  // (300 modules + 4 utils + 1 entry) whose entire graph is served across the
  // JavaScript boundary -- 608 resolveId plus 305 load callbacks per rebuild,
  // both counted -- plus materialising the rendered output for the caller.
  // `nohooks` names the absence of an OUTPUT hook, not the absence of hooks.
  // Consequence for this table: only the variant-to-variant DELTAS isolate one
  // hook's retention, because that shared baseline cancels out of them. A move
  // in `nohooks` itself means the shared build/marshalling path changed.
  nohooks: { measured: 0.195, budget: 0.244 },
  // A generateBundle hook that READS the chunks it is given. Costs NOTHING
  // extra, because the per-invocation bundle copy IS released when the hook
  // returns -- the contract pinned by
  // packages/rolldown/tests/workerd-output-ownership.test.ts. This variant is
  // the regression guard on that release: if it ever starts tracking well
  // above the baseline, the generateBundle copy stopped being freed.
  generatebundle: { measured: 0.18, budget: 0.225 },
  // Receiving a BindingRenderedChunk in renderChunk. The chunk argument is
  // snapshot-and-released per invocation now, so this is baseline plus the
  // cost of installing the extra hook, NOT a payload retention (it no longer
  // scales with output size; on the c0c2f0c2 pin it sits ~0.011 above
  // `nohooks` -- see the workload-resolution control below).
  // If this variant climbs back toward 0.55, the renderChunk eager release
  // broke.
  'renderchunk-nomodules': { measured: 0.206, budget: 0.258 },
  // ...plus reading `chunk.modules`, which marshals one BindingRenderedModule
  // per module. Identical to `renderchunk-nomodules` today because the module
  // boxes are snapshot-and-released too. The shared band alone cannot see the
  // module-box cost returning (0.021 fits under the ceiling), so the
  // `moduleBoxDelta` control below asserts the two rows stay within two page
  // quanta of each other.
  renderchunk: { measured: 0.206, budget: 0.258 },
};

// Every variant is enforced as a two-sided BAND, not just a ceiling. `budget`
// catches the leak growing; `measured * SLOPE_FLOOR_RATIO` catches it
// SHRINKING, which is the day this table has to be re-derived. Without a floor
// a landed fix leaves a stale ceiling behind that a future regression can grow
// back into with CI green the whole way.
//
// The ratio has to sit between the two things it separates:
//   * NOISE -- the widest spread ever seen is `generatebundle`'s GC-timing
//     band of 0.033 (0.309-0.342, measured across eight 20-round runs of the
//     ad23bdeb base-pin derivation; the c0c2f0c2 CI attempts repeated every
//     value exactly). 25% of the smallest measured slope is 0.045
//     MiB/rebuild -- still ~1.4x that widest spread.
//   * SIGNAL -- proven in anger: the hook-input eager release landing dropped
//     `renderchunk-nomodules` from 0.528 to 0.313 (12-round window), 21%
//     below its then-floor of 0.396, which is exactly the detection that
//     forced this table's 2026-08-09 re-derivation. A shift of that size
//     stays detectable.
const SLOPE_FLOOR_RATIO = 0.75;

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

const {
  multiModuleBuild,
  errorSurface,
  lifecycle,
  concurrency,
  capabilities,
  fireAndForgetLoad,
  failedBuildReuse,
} = behavior.cases;

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
  `  [1/8] multi-module build       ok  (${multiModuleBuild.moduleCount} modules in chunk, ` +
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
assert.equal(
  errorSurface.liveDelta,
  0,
  'a failed build must leave the caller-owned instance disposable',
);
// The caller-owned half above cannot prove failure CLEANUP -- its dispose() is
// explicit. `build({ module })` is the path that can actually leak: it creates
// a private instance only build() itself can dispose, so a failed build that
// forgets strands an entire Wasm instance inside the 128 MiB isolate cap.
const ownedFailure = errorSurface.ownedFailure;
assert.ok(ownedFailure.caught, 'the failing build({module}) must reject');
assert.equal(ownedFailure.caught.isError, true, 'the rejection must be a real Error');
assert.match(ownedFailure.caught.message, /definitely-not-resolvable/);
assert.deepEqual(
  ownedFailure.caught.ansiAt,
  [],
  `ANSI escapes leaked into: ${ownedFailure.caught.ansiAt.join(', ')}`,
);
assert.equal(
  ownedFailure.liveDelta,
  0,
  'a FAILED build({module}) must still dispose the private instance it created',
);
console.log(
  `  [2/8] error surface            ok  (${caught.errorCount} error(s), code frame ` +
    `inlined in message, ${caught.frameCount} separate .frame, no ANSI, ` +
    `failed build({module}) self-disposed) ${elapsed()}`,
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
console.log(`  [3/8] lifecycle contracts      ok  ${elapsed()}`);

// --- CASE 4: concurrency and admission -------------------------------------
// "Both promises fulfilled" is not success: empty output, cross-wired output,
// or one build silently producing nothing would all satisfy it. The two builds
// therefore run DISTINCT entries with distinct computed totals, and each half
// is validated on its own -- its own chunk, executed the same way CASE 1
// executes its chunk, and its own independent hook trace.
const concurrentTotals = [];
for (const [label, side, entryId, expectedTotal, expectedTag] of [
  ['first', concurrency.concurrentSameInstance.first, 'fan:entry-a.js', 1390, 'a'],
  ['second', concurrency.concurrentSameInstance.second, 'fan:entry-b.js', 2390, 'b'],
]) {
  assert.equal(side.ok, true, `${label} concurrent build failed: ${JSON.stringify(side)}`);
  assert.equal(
    side.loadedCount,
    26,
    `${label} concurrent build served ${side.loadedCount} modules, expected its whole 26-module fan`,
  );
  assert.deepEqual(
    side.entriesLoaded,
    [entryId],
    `${label} concurrent build's hooks saw entries ${JSON.stringify(side.entriesLoaded)}; ` +
      `each build must drive its OWN trace over ${entryId} alone`,
  );
  const built = await import(
    `data:text/javascript;base64,${Buffer.from(side.code).toString('base64')}`
  );
  assert.equal(built.tag, expectedTag, `${label} concurrent build returned the wrong chunk`);
  assert.equal(
    built.total,
    expectedTotal,
    `${label} concurrent chunk computed total=${built.total}, expected ${expectedTotal}`,
  );
  concurrentTotals.push(built.total);
}
assert.notEqual(
  concurrency.concurrentSameInstance.first.fileName,
  concurrency.concurrentSameInstance.second.fileName,
  'the two concurrent builds must emit their own distinctly named chunks',
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
console.log(
  `  [4/8] concurrency + admission  ok  (concurrent totals ${concurrentTotals.join('/')}) ` +
    `${elapsed()}`,
);

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
console.log(
  `  [5/8] capabilities             ok  (target=${capabilities.target}, ` +
    `flavor=${capabilities.flavor}, threads=${capabilities.threads}) ${elapsed()}`,
);

// --- CASE 6: fire-and-forget this.load() -----------------------------------
// The documented un-awaited `this.load()` (and the cycle-load-error self-load
// shape) keeps a napi borrow on the plugin-context box past the hook's
// settle. The eager-release wiring must defer the box's release to the call's
// completion instead of tripping the napi borrow checker inside the hook
// wrapper's `finally` and failing the build in the plugin's name.
assert.equal(fireAndForgetLoad.chunkCount, 1, 'the fire-and-forget this.load() build must succeed');
assert.equal(
  fireAndForgetLoad.awaitedCodeType,
  'string',
  'an awaited this.load() must still deliver the module info',
);
assert.equal(fireAndForgetLoad.liveDelta, 0, 'build({module}) must dispose its private instance');
console.log(`  [6/8] fire-and-forget load     ok  ${elapsed()}`);

// --- CASE 7: failed-build reuse --------------------------------------------
// The native invalidateJsSideCache callback only fires after a successful
// generate, so a failed build must release its retained option boxes at its
// own settle. The case runs 10 failing builds carrying a ~1 MiB banner inside
// the normalized options on one reusable instance: a stranded per-build
// options box shows up as ~1 MiB of arena growth per failed build, while the
// released path stays near-flat (bound 0.25 leaves 4x margin each way).
assert.equal(failedBuildReuse.unexpectedSuccess, undefined, 'the failing builds must fail');
assert.equal(failedBuildReuse.failures, 10, 'every failing build must reject');
assert.equal(failedBuildReuse.sawOptions, 10, 'buildStart must read the options box every build');
assert.ok(
  failedBuildReuse.slopeMiBPerFailedBuild < 0.25,
  `failed builds must not retain their options graph: ` +
    `${failedBuildReuse.slopeMiBPerFailedBuild.toFixed(4)} MiB/failed-build (leak signal ~1.0)`,
);
assert.equal(
  failedBuildReuse.recoveredChunkCount,
  1,
  'the instance must still build successfully after repeated failed builds',
);
assert.equal(behavior.finalLiveInstances, behavior.baselineLiveInstances, 'an instance leaked');
console.log(
  `  [7/8] failed-build reuse       ok  ` +
    `(${failedBuildReuse.slopeMiBPerFailedBuild.toFixed(4)} MiB/failed-build) ${elapsed()}`,
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
const firstSamples = {};
const failures = [];
for (const variant of Object.keys(MEMORY_BUDGETS)) {
  const report = await dispatch(
    `/memory?variant=${variant}&rounds=${rounds}&modules=${slopeModules}`,
  );
  // A teardown that only breaks after N rebuilds is exactly the regression
  // this part is shaped to find, so the worker reports build failure and
  // DISPOSE failure separately and both are fatal here. A run whose rounds all
  // succeeded but whose dispose() threw must not go on to pass a budget.
  if (report.error || report.disposeError) {
    throw new Error(
      `memory variant ${variant} failed:\n` +
        [report.error, report.disposeError].filter(Boolean).join('\n'),
    );
  }
  assert.equal(report.memPerRound.length, rounds, `variant ${variant} did not run every round`);
  assert.equal(report.disposed, true, `variant ${variant} left its instance undisposed`);
  assert.equal(
    report.liveDelta,
    0,
    `variant ${variant} did not return the live-instance count to its pre-case value ` +
      `(delta ${report.liveDelta})`,
  );
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
  firstSamples[variant] = report.memPerRound[0];

  // POSITIVE CONTROL -- prove the telemetry is ALIVE before trusting a slope.
  // Every budget check below is `slope <= budget`, so the moment
  // `instance.memoryBytes` starts returning a constant (API change, wiring
  // regression, a stubbed accessor) every slope reads 0.000, every check
  // passes, and the whole memory half of this lane reports green while
  // measuring nothing. These checks make that fail loudly instead.
  //
  // They are asserted for EVERY variant because every variant demonstrably
  // grows: the shared baseline (see MEMORY_BUDGETS.nohooks) retains on its own,
  // so growth does not depend on which output hook is installed. They also
  // survive the BindingRenderedChunk fix, which is expected to pull the
  // renderChunk variants down TO that baseline, not to zero.
  const distinctSamples = new Set(report.memPerRound).size;
  report.memPerRound.forEach((bytes, index) => {
    // Wasm linear memory can only grow, and never below the pages the instance
    // declared at creation. A sample outside that is not a measurement.
    assert.ok(
      Number.isSafeInteger(bytes) && bytes >= report.declaredInitialMemoryBytes,
      `variant ${variant} round ${index + 1} reported memoryBytes=${bytes}, which is not a ` +
        `live linear-memory size (>= ${report.declaredInitialMemoryBytes})`,
    );
    assert.ok(
      index === 0 || bytes >= report.memPerRound[index - 1],
      `variant ${variant} memoryBytes SHRANK at round ${index + 1} ` +
        `(${report.memPerRound[index - 1]} -> ${bytes}); Wasm linear memory cannot shrink, ` +
        'so this accessor is no longer reporting it',
    );
  });
  // `slopeMiBPerRound` already asserted this window is at least 2 samples wide.
  const post = report.memPerRound.slice(warmupRounds);
  assert.ok(
    post.at(-1) > post[0],
    `variant ${variant} retained NOTHING across ${post.length} rebuilds ` +
      `(${post[0]} -> ${post.at(-1)} bytes). instance.memoryBytes is not tracking this ` +
      'instance any more, so the slope below would read 0.000 and pass every budget. ' +
      'Fix the telemetry; do not relax this check.',
  );
  assert.ok(
    distinctSamples >= 3,
    `variant ${variant} reported only ${distinctSamples} distinct memoryBytes value(s) ` +
      `across ${rounds} rounds; the accessor looks stubbed or quantised into uselessness`,
  );

  const { measured, budget } = MEMORY_BUDGETS[variant];
  const floor = measured * SLOPE_FLOOR_RATIO;
  const verdict =
    measureOnly || budget === null
      ? 'measured'
      : slope > budget
        ? 'OVER BUDGET'
        : slope < floor
          ? 'UNDER FLOOR'
          : 'ok';
  console.log(
    `  [8/8] memory ${variant.padEnd(21)} ${verdict.padEnd(11)} ` +
      `${slope.toFixed(3)} MiB/rebuild` +
      (budget === null ? '' : ` (band ${floor.toFixed(3)}-${budget.toFixed(3)})`) +
      `  [${report.memFirstMiB} -> ${report.memLastMiB} MiB over ${rounds} rounds]`,
  );
  if (!measureOnly && budget !== null && slope > budget) {
    failures.push(
      `memory variant '${variant}' retained ${slope.toFixed(3)} MiB/rebuild, ` +
        `budget is ${budget.toFixed(3)} MiB/rebuild. A hook-input eager-release path ` +
        '(dropInner wiring) regressed, or a new leak was introduced. Investigate before ' +
        'raising this budget.',
    );
  }
  if (!measureOnly && slope < floor) {
    failures.push(
      `memory variant '${variant}' retained ${slope.toFixed(3)} MiB/rebuild, well under the ` +
        `${measured.toFixed(3)} recorded for it (floor ${floor.toFixed(3)}). THIS IS GOOD ` +
        'NEWS: something now releases memory this suite still budgets for. Re-run this ' +
        'script with `--measure` and update BOTH `measured` and `budget` for every variant ' +
        'in MEMORY_BUDGETS to the new numbers. Leaving the old ceiling in place would let a ' +
        'future regression grow back into it with this lane green the whole way.',
    );
  }
}

// The last positive control, and the strongest one: the telemetry must RESOLVE
// a workload difference, not merely move. `renderchunk-nomodules` differs from
// `nohooks` by exactly one thing -- a hook that RECEIVES a rendered chunk --
// and that difference has to show up in the numbers. On the c0c2f0c2 pin the
// resolution lives entirely in the slope gap: CI run 31322883081 measured
// +0.011 MiB/rebuild at --rounds=20 with a first-round footprint gap of
// 0.000 (installing the extra hook no longer costs pages up front on this
// binding). The slope threshold is 0.007 -- two measurement quanta, one
// 64 KiB page across the 17 post-warmup intervals at --rounds=20 being
// ~0.0037 MiB/rebuild -- and both gaps are checked as MAGNITUDES because the
// arena tail can flip the sign. A counter that grows but ignores what the
// build actually did would show NEITHER -- and would still satisfy every
// per-variant check above.
if (!measureOnly) {
  const hookSlopeDelta = slopes['renderchunk-nomodules'] - slopes.nohooks;
  const hookFirstDelta = (firstSamples['renderchunk-nomodules'] - firstSamples.nohooks) / MiB;
  if (Math.abs(hookSlopeDelta) < 0.007 && Math.abs(hookFirstDelta) < 0.1) {
    failures.push(
      `the renderChunk workload is no longer distinguishable from the baseline: ` +
        `'renderchunk-nomodules' ${slopes['renderchunk-nomodules'].toFixed(3)} vs 'nohooks' ` +
        `${slopes.nohooks.toFixed(3)} MiB/rebuild (gap ${hookSlopeDelta.toFixed(3)}, threshold ` +
        `0.007) and a first-round footprint gap of ${hookFirstDelta.toFixed(3)} MiB (measured ` +
        '0.000 on the c0c2f0c2 pin, so the slope gap carries the resolution). EITHER ' +
        'instance.memoryBytes stopped tracking what the build does -- in ' +
        'which case every slope above is meaningless -- OR the per-hook cost itself was ' +
        'eliminated, in which case re-run with `--measure` and re-baseline the whole ' +
        'MEMORY_BUDGETS table.',
    );
  }

  const moduleBoxDelta = slopes.renderchunk - slopes['renderchunk-nomodules'];
  if (moduleBoxDelta > 0.014) {
    failures.push(
      `reading chunk.modules costs ${moduleBoxDelta.toFixed(3)} MiB/rebuild over not reading it ` +
        `('renderchunk' ${slopes.renderchunk.toFixed(3)} vs 'renderchunk-nomodules' ` +
        `${slopes['renderchunk-nomodules'].toFixed(3)}); the rows were identical at baseline. ` +
        'Either `snapshotChunkModules` stopped covering the hook path, or the ' +
        'BindingRenderedModule boxes leak again (historical cost: 0.021 MiB/rebuild).',
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
