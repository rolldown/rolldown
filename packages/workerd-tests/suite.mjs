// End-to-end suite for `@rolldown/browser/workerd` inside real workerd (via
// Miniflare): multi-module build, error surface, lifecycle/admission, memory slope.
//
// Runs against `packages/browser/dist`; browser-tests' prepare-fixture.mjs
// checks the packed package's `workerd` export condition.
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stripVTControlCharacters } from 'node:util';

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
const rounds = Number(args.get('--rounds') ?? 20);
const slopeModules = 300;
// Rounds discarded before measuring: the first rebuilds on a fresh instance pay
// one-time allocations that are not a per-rebuild leak.
const warmupRounds = 2;
// `--measure` reports the slopes without enforcing the budgets, to re-derive them.
const measureOnly = flags.has('--measure');
// The smallest slope a run can show: one 64 KiB Wasm page spread over the
// post-warmup intervals (~0.0037 MiB/rebuild at --rounds=20).
const quantum = 65536 / (rounds - warmupRounds - 1) / MiB;

// How deep `renderchunk-stacked` stacks the renderChunk hook, to give the
// workload-resolution control at the bottom a signal above page noise. Only the
// invocation count moves retention (the ~25 KiB chunk is under one page). Depth 6
// sits past the dlmalloc arena step between depths 3 and 4, on a flat plateau, so
// one depth either way gives the same signal.
const STACKED_RENDER_CHUNK_PLUGINS = 6;
const renderChunkDepth = (variant) =>
  variant === 'renderchunk-stacked' ? STACKED_RENDER_CHUNK_PLUGINS : 1;

// What one round owes `transform`: the whole slope graph (`slopeModules` modules
// plus 4 utils and 1 entry) AND rolldown's injected runtime module, which
// transform sees even though no `load` ever serves it.
const TRANSFORMED_MODULES_PER_ROUND = slopeModules + 5 + 1;
// Hook invocations one round owes. The output hooks run once per chunk, times
// their stack depth; the `transform-ast*` rows hook the BUILD side instead, so
// their count is per-module.
const hookCallsPerRound = (variant) =>
  variant.startsWith('transform-ast') ? TRANSFORMED_MODULES_PER_ROUND : renderChunkDepth(variant);

// ---------------------------------------------------------------------------
// MEMORY BUDGETS -- MiB of Wasm linear memory retained per identical rebuild on
// ONE reusable instance; they catch a hook-input eager-release path dropping out.
// Each budget is the slope measured on the DEBUG wasm at --rounds=20 (the count
// ci.yml runs) plus ~25% headroom; re-derive with `--measure`.
const MEMORY_BUDGETS = {
  // Baseline, not a bare rebuild: every variant installs `slopeGraphPlugin`, so
  // this row includes serving the 305-module graph across the JS boundary. Only
  // deltas between rows isolate one hook's retention.
  nohooks: { budget: 0.249 }, // measured 0.199
  // A generateBundle hook that reads its chunks. Its per-invocation bundle copy
  // is released when the hook returns (pinned by
  // packages/rolldown/tests/workerd-output-ownership.test.ts); rising above
  // baseline means that copy stopped being freed.
  generatebundle: { budget: 0.228 }, // measured 0.182
  // renderChunk receives a BindingRenderedChunk, snapshot-and-released per
  // invocation. Climbing toward 0.55 means that eager release broke.
  'renderchunk-nomodules': { budget: 0.263 }, // measured 0.21
  // ...plus reading `chunk.modules` (one BindingRenderedModule per module, also
  // released). The band cannot see that cost return (0.021 fits under it); the
  // `moduleBoxDelta` control below can. Both rows stay at depth 1 because
  // stacking makes each jitter +-2 quanta and eats that control's margin.
  renderchunk: { budget: 0.263 }, // measured 0.21
  // The `renderchunk-nomodules` hook stacked STACKED_RENDER_CHUNK_PLUGINS deep:
  // a broken release shows 6x here, and its gap over `nohooks` feeds the
  // workload-resolution control at the bottom.
  'renderchunk-stacked': { budget: 0.432 }, // measured 0.346
  // The only BUILD-side row (306 hook calls a round): a transform hook reading
  // `meta.ast`. oxc's `ParseResult` is an upstream napi class whose native
  // storage is freed only by reading each field, which `wrapParseResult` in
  // packages/rolldown/src/utils/parse.ts does.
  'transform-ast': { budget: 0.331 }, // measured 0.265
  // The failing half: `this.parse()` on broken source throws, but oxc fills
  // `ParseResult` first, so the throwing branch owes the same drains, and they
  // must run before the error check.
  'transform-ast-error': { budget: 0.331 }, // measured 0.265
};

// ---------------------------------------------------------------------------
// Artifact preflight: a missing artifact is a hard failure naming the command
// that builds it, never a silent skip.
const workerdEntry = path.join(distDir, 'workerd.browser.mjs');
const wasmBinary = path.join(distDir, 'rolldown-binding.wasm32-wasip1.wasm');
const workerSource = path.join(repoRoot, 'packages/workerd-tests/worker.js');

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
 * Run one request against a FRESH workerd isolate. Module names are paths
 * relative to `modulesRoot`, so the worker's sibling specifiers resolve to the
 * real dist artifacts; `contents` lets the worker source live outside `dist/`.
 */
async function dispatch(route) {
  const miniflare = new Miniflare({
    compatibilityDate: '2026-06-01',
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
  sourcemapUrl,
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
assert.equal(multiModuleBuild.generateBundleCalls, 1, 'generateBundle must run exactly once');
assert.equal(multiModuleBuild.assetCount, 1, 'generateBundle emitFile must add one asset');
assert.deepEqual(
  JSON.parse(multiModuleBuild.manifestSource),
  { entries: [multiModuleBuild.fileName] },
  'the emitted manifest must describe the real bundle',
);
assert.equal(multiModuleBuild.liveDelta, 0, 'build({module}) must dispose its private instance');

// Prove the OUTPUT is correct, not merely well-shaped: execute the chunk workerd
// generated and check the value it computes. workerd forbids dynamic code
// evaluation, so this half necessarily runs in Node.
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
  `  [1/9] multi-module build       ok  (${multiModuleBuild.moduleCount} modules in chunk, ` +
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
// The code frame is inlined into `message` (no separate `.frame`). It keeps the
// ANSI colors the native diagnostic renders.
const plainMessage = stripVTControlCharacters(caught.message);
assert.match(plainMessage, /UNRESOLVED_IMPORT/);
assert.match(plainMessage, /fan:bad-entry\.js:1:19/, 'the code frame must carry a location');
assert.match(
  plainMessage,
  /import \{ q \} from '\.\/definitely-not-resolvable\.js';/,
  'the code frame must quote the offending source line',
);
assert.equal(errorSurface.reuse.ok, true, 'the same instance must build after a failed build');
assert.equal(errorSurface.reuse.loadedCount, 26);
assert.equal(errorSurface.disposed, true);
assert.equal(
  errorSurface.liveDelta,
  0,
  'a failed build must leave the caller-owned instance disposable',
);
// `build({ module })` owns a private instance only build() can dispose, so a
// failed build that forgets strands a whole Wasm instance in the 128 MiB isolate.
const ownedFailure = errorSurface.ownedFailure;
assert.ok(ownedFailure.caught, 'the failing build({module}) must reject');
assert.equal(ownedFailure.caught.isError, true, 'the rejection must be a real Error');
assert.match(ownedFailure.caught.message, /definitely-not-resolvable/);
assert.equal(
  ownedFailure.liveDelta,
  0,
  'a FAILED build({module}) must still dispose the private instance it created',
);
console.log(
  `  [2/9] error surface            ok  (${caught.errorCount} error(s), code frame ` +
    `inlined in message, ${caught.frameCount} separate .frame, ` +
    `failed build({module}) self-disposed) ${elapsed()}`,
);

// --- CASE 3: lifecycle contracts -------------------------------------------
for (const [label, refused] of [
  ['while a build is parked in load', lifecycle.disposeWhileBuilding],
  ['from closeBundle (the bundle is still open)', lifecycle.disposeInCloseBundle],
]) {
  assert.equal(refused?.ok, false, `dispose() ${label} must be refused`);
  assert.match(
    refused.error.message,
    /Cannot dispose this workerd Rolldown instance with 1 active binding operation;/,
  );
}
assert.equal(lifecycle.slowBuild.ok, true, 'the refused dispose must not break the build');
assert.equal(lifecycle.otherInstanceAfterBuild.ok, true, 'a settled build must release the slot');
assert.equal(lifecycle.disposeAfterBuild.ok, true, 'dispose() must succeed once the build settled');
assert.equal(lifecycle.memoryAfterDispose.ok, false, '.memory must throw after dispose()');
assert.match(lifecycle.memoryAfterDispose.error.message, /has been disposed/);
assert.equal(lifecycle.buildAfterDispose.ok, false, 'build() must reject after dispose()');
assert.match(lifecycle.buildAfterDispose.error.message, /has been disposed/);
assert.equal(lifecycle.instanceADisposed, true);
assert.equal(lifecycle.instanceBDisposed, true);
assert.equal(lifecycle.liveDelta, 0, 'the lifecycle case must leak no instance');
console.log(`  [3/9] lifecycle contracts      ok  ${elapsed()}`);

// --- CASE 4: concurrency and admission -------------------------------------
// Both promises fulfilling is not success: each build runs a distinct entry with
// a distinct total, checked through its own executed chunk and hook trace.
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
  `  [4/9] concurrency + admission  ok  (concurrent totals ${concurrentTotals.join('/')}) ` +
    `${elapsed()}`,
);

// --- CASE 5: capabilities as reported INSIDE workerd -----------------------
assert.equal(capabilities.refusedCode, 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE');
assert.deepEqual(
  capabilities.instanceKeys,
  ['dispose', 'disposed', 'memory', 'memoryBytes'],
  'the instance handle must not expose the raw binding exports',
);
assert.equal(capabilities.target, 'wasi');
assert.equal(capabilities.flavor, 'CurrentThread');
assert.equal(capabilities.threads, false, 'workerd must never report thread support');
assert.equal(capabilities.wasi, true);
assert.equal(capabilities.watchSupported, false, 'watch is unsupported in workerd');
assert.equal(capabilities.devSupported, false, 'dev is unsupported in workerd');
assert.equal(capabilities.watchExported, false, 'the workerd entry must not export watch()');
assert.deepEqual(
  capabilities.entryExports,
  ['build', 'createInstance', 'getWorkerdRuntimeStats'],
  'the workerd entry export surface changed',
);
assert.ok(capabilities.memoryBytes > 0);
console.log(
  `  [5/9] capabilities             ok  (target=${capabilities.target}, ` +
    `flavor=${capabilities.flavor}, threads=${capabilities.threads}) ${elapsed()}`,
);

// --- CASE 6: fire-and-forget this.load() -----------------------------------
// An un-awaited `this.load()` keeps a napi borrow on the plugin-context box past
// the hook's settle, so its eager release must wait for the call. Fails if the
// release trips the napi borrow checker and fails the build.
assert.equal(fireAndForgetLoad.chunkCount, 1, 'the fire-and-forget this.load() build must succeed');
assert.equal(
  fireAndForgetLoad.awaitedCodeType,
  'string',
  'an awaited this.load() must still deliver the module info',
);
assert.equal(fireAndForgetLoad.liveDelta, 0, 'build({module}) must dispose its private instance');
console.log(`  [6/9] fire-and-forget load     ok  ${elapsed()}`);

// --- CASE 7: failed-build reuse --------------------------------------------
// A stranded options box grows ~1 MiB per failed build (see
// `caseFailedBuildReuse` in worker.js); the 0.25 bound sits 4x under that.
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
  `  [7/9] failed-build reuse       ok  ` +
    `(${failedBuildReuse.slopeMiBPerFailedBuild.toFixed(4)} MiB/failed-build) ${elapsed()}`,
);

// --- CASE 8: map.toUrl() without a Buffer global ---------------------------
// The suite's workerd config has no Node compatibility; if a `Buffer` global
// appears, this case no longer covers the path it exists for.
assert.equal(sourcemapUrl.bufferGlobal, 'undefined', 'workerd must run this case without `Buffer`');
const dataUrlPrefix = 'data:application/json;charset=utf-8;base64,';
assert.ok(
  sourcemapUrl.url.startsWith(dataUrlPrefix),
  `toUrl() returned ${sourcemapUrl.url.slice(0, 80)}`,
);
const decodedMap = JSON.parse(
  Buffer.from(sourcemapUrl.url.slice(dataUrlPrefix.length), 'base64').toString('utf8'),
);
assert.deepEqual(decodedMap, JSON.parse(sourcemapUrl.mapString), 'toUrl() must encode toString()');
assert.deepEqual(
  decodedMap.sourcesContent,
  [sourcemapUrl.source],
  'the non-ASCII source must round-trip as UTF-8',
);
assert.ok(
  Buffer.byteLength(sourcemapUrl.mapString, 'utf8') > 3 * 0x7ffe,
  'the map must span several 0x7ffe-byte encoder chunks',
);
console.log(`  [8/9] sourcemap toUrl          ok  ${elapsed()}`);

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
    `/memory?variant=${variant}&rounds=${rounds}&modules=${slopeModules}` +
      `&hooks=${renderChunkDepth(variant)}`,
  );
  // A dispose() failure is fatal too: a teardown that breaks only after N
  // rebuilds must not pass a budget.
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
    // The driver's own arithmetic, so a worker that installed fewer hooks or
    // transformed a smaller graph cannot satisfy it.
    const expectedHookCalls = rounds * hookCallsPerRound(variant);
    assert.equal(
      report.hookCalls,
      expectedHookCalls,
      `variant ${variant} ran ${report.hookCalls} hook calls, expected ${expectedHookCalls}`,
    );
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

  // POSITIVE CONTROL -- prove the telemetry is alive. If `instance.memoryBytes`
  // went constant, every slope would read 0 and pass its budget. Every variant
  // grows (the shared `nohooks` baseline retains on its own), so all are checked.
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

  const { budget } = MEMORY_BUDGETS[variant];
  const verdict = measureOnly ? 'measured' : slope > budget ? 'OVER BUDGET' : 'ok';
  console.log(
    `  [9/9] memory ${variant.padEnd(21)} ${verdict.padEnd(11)} ` +
      `${slope.toFixed(3)} MiB/rebuild` +
      ` (budget ${budget.toFixed(3)})` +
      `  [${report.memFirstMiB} -> ${report.memLastMiB} MiB over ${rounds} rounds]`,
  );
  if (!measureOnly && slope > budget) {
    failures.push(
      `memory variant '${variant}' retained ${slope.toFixed(3)} MiB/rebuild, ` +
        `budget is ${budget.toFixed(3)} MiB/rebuild. A hook-input eager-release path ` +
        '(dropInner wiring) regressed, or a new leak was introduced. Investigate before ' +
        'raising this budget.',
    );
  }
}

// The strongest positive control: the telemetry must RESOLVE a workload
// difference, not merely move. A counter that grows but ignores the build passes
// every per-variant check above but fails here. The `renderchunk-stacked` gap
// over `nohooks` measured 38-40 quanta with a 0 first-round gap; the 0.030
// threshold (~8 quanta) clears page jitter on both rows and still fails if the
// gap collapses to the single-hook 3 quanta. Gaps are compared as magnitudes
// because the arena tail can flip the sign.
if (!measureOnly) {
  const hookSlopeDelta = slopes['renderchunk-stacked'] - slopes.nohooks;
  const hookFirstDelta = (firstSamples['renderchunk-stacked'] - firstSamples.nohooks) / MiB;
  if (Math.abs(hookSlopeDelta) < 0.03 && Math.abs(hookFirstDelta) < 0.1) {
    failures.push(
      `the renderChunk workload is no longer distinguishable from the baseline: ` +
        `'renderchunk-stacked' ${slopes['renderchunk-stacked'].toFixed(3)} vs 'nohooks' ` +
        `${slopes.nohooks.toFixed(3)} MiB/rebuild (gap ${hookSlopeDelta.toFixed(3)}, threshold ` +
        `0.030) and a first-round footprint gap of ${hookFirstDelta.toFixed(3)} MiB (threshold ` +
        '0.1). EITHER ' +
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
        'BindingRenderedModule boxes leak again.',
    );
  }
}

if (measureOnly) {
  console.log(`\nMEASURED SLOPES (--measure, budgets not enforced):`);
  console.log(JSON.stringify(slopes, null, 2));
  // The two controls above, in the unit they are calibrated in, so a re-baseline
  // can read its margin off the run instead of recomputing it.
  const gap = slopes['renderchunk-stacked'] - slopes.nohooks;
  const boxDelta = slopes.renderchunk - slopes['renderchunk-nomodules'];
  console.log(
    `workload-resolution gap ${gap.toFixed(4)} (${(gap / quantum).toFixed(1)} quanta, ` +
      `threshold 0.030) | moduleBoxDelta ${boxDelta.toFixed(4)} ` +
      `(${(boxDelta / quantum).toFixed(1)} quanta, ceiling 0.014) | quantum ` +
      `${quantum.toFixed(4)} MiB/rebuild`,
  );
} else if (failures.length > 0) {
  throw new Error(`\n${failures.join('\n')}`);
}

console.log(
  `\nOK: @rolldown/browser/workerd builds, fails, disposes, admits and retains ` +
    `memory as expected inside real workerd (${elapsed()})`,
);
