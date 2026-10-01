const ASYNC_RUNTIME_HOST_EXPORTS = [
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
] as const;

export type BindingLoaderModuleFormat = 'commonjs' | 'esm';

const WASI_CJS_CREATE_CONTEXT_IMPORT =
  "const { createContext: __emnapiCreateContext } = require('@emnapi/runtime')\n";
const WASI_ESM_CREATE_CONTEXT_IMPORT =
  "import { createContext as __emnapiCreateContext } from '@emnapi/runtime'\n";
const WASI_CONTEXT_SUPPRESS_DESTROY = '__emnapiContext.suppressDestroy()';
const WASI_CONTEXT_PREPARE_CLEANUP_FLAG = 'let __emnapiWasmEnvCleanupPrepared = false\n';
// A raw `context.destroy()` must run the wasm-side cleanup preparation first:
// it cancels pending napi async work while the env can still call into
// JavaScript, so deferreds reject instead of panicking on a dead threadsafe
// function. Upstream owns the wrapper since `@napi-rs/cli` 3.10.0
// (napi-rs#3514); these two anchors are the guard that a cli bump has not
// dropped it again.
const WASI_CONTEXT_DESTROY_WRAP_HELPER = `function __wrapEmnapiContextDestroyForSettlement(
  context,
  prepareEnvCleanup,
  isPreparingEnvCleanup,
) {`;
// Indentation differs per flavor (2 spaces in the browser ESM loaders, 4 in
// the Node CommonJS ones), so this anchor is matched whitespace-normalized.
const WASI_CONTEXT_DESTROY_WRAP_WIRING = `__emnapiContext = __wrapEmnapiContextDestroyForSettlement(
  __emnapiCreateContext({ autoDestroy: false }),
  __prepareWasmEnvCleanup,
  __isPreparingWasmEnvCleanup,
)`;
// Settlement barrier: the cleanup preparation must precede the context
// destroy, or the TSFN cleanup hook discards pending napi async work. Since
// `@napi-rs/cli` 3.10.5 (napi-rs#3541) a reentrancy guard sits between the two
// and returns while the barrier is still running.
const WASI_CONTEXT_DESTROY_SETTLEMENT = `  __prepareWasmEnvCleanup()
  if (__isPreparingWasmEnvCleanup()) {
`;
// The disposal chain runs prepare -> drain -> destroy -> worker termination
// and publishes Symbol.for('napi.rs.wasi.dispose') on the binding exports.
const WASI_DISPOSAL_CHAIN_SIGNATURES = [
  'function __prepareWasmEnvCleanup() {',
  // `@napi-rs/cli` >= 3.10.5 (napi-rs#3541): the yielding two-phase cleanup
  // (begin, event-loop turns while work is pending, finish) used by dispose.
  'function __prepareWasmEnvCleanupWithTurns() {',
  'function __drainWasmEnvCleanup() {',
  'function __destroyEmnapiContext() {',
  'function __terminateWasiWorkers() {',
  'function __startWasiDisposal() {',
  'function __disposeWasiBinding() {',
  'function __publishWasiDispose(exports) {',
  'function __rollbackWasiInitialization() {',
] as const;
const WASI_DISPOSE_PUBLICATION = '__publishWasiDispose(__napiModule.exports)';
// Both async teardown waits: the disposal chain must settle a thenable
// context destroy and collect thenable worker terminations before it
// completes, or teardown failures and retry ownership are lost.
const WASI_ASYNC_TEARDOWN_WAITS = [
  {
    label: 'WASI thenable-aware context destroy',
    snippet: `  const destroyResult = __destroyEmnapiContext()
  if (__isThenable(destroyResult)) {
`,
  },
  {
    label: 'WASI thenable-aware worker termination',
    snippet: `    if (__isThenable(result)) {
      pending.push(
        Promise.resolve(result).then(
`,
  },
] as const;
const WASI_EXIT_LISTENER_HELPER = 'function __registerWasiExitListener() {';
// Worker-crash latch, threaded Node flavor only (`rolldown-binding.wasi.cjs` +
// `wasi-worker.mjs`; vendored `@napi-rs/cli` packed from napi-rs 22ee6c8d, branch
// fix/wasi-crash-latch (napi-rs#3552), until a cli release carries it).
// After a pool worker's wasm thread dies, no teardown may re-enter wasm: the
// env cleanup waits for the dead thread's work to go idle in a raw
// `memory.atomic.wait32`, which blocks the main thread forever and keeps a JS
// SIGTERM listener from ever running. The worker sets a shared flag before
// emnapi reports the crash. Both disposers check it before any other step: the
// exit listener then only terminates the workers, and the public
// `Symbol.for('napi.rs.wasi.dispose')` disposer terminates them and rejects
// (latched) instead of draining async work into the cleanup barrier, where its
// promise would never settle. Before it terminates them, it unrefs emnapi's
// waiting-request port: the dead thread's requests never finish, so that port
// would keep the process alive for good.
// A crash that lands while a public disposal is already running is caught by
// the poll turns and step boundaries (`__abortWasiDisposalIfThreadCrashed`);
// the in-flight promise then settles through the crash disposal, and later
// calls get that same promise. The initialization rollback runs the same polls
// and steps and is stopped the same way. The worker writes its error and
// threadId into a shared crash report before it raises the flag, so the
// rejection's `cause` and `workerThreadId` do not depend on the 'error' event,
// which terminating the workers can drop.
// Addon crash flag bridge: the loader flag above only guards the next entry into
// wasm. A cleanup call already inside wasm (the exit teardown's
// `napi_prepare_wasm_env_cleanup`, the disposer's `..._finish`) waits on the dead
// thread in short slices that check one word of the shared wasm memory. In
// `beforeInit`, before any registration code runs, the loader reads that word's
// address from `napi_wasm_thread_crash_flag_address` and passes each pool worker
// an `Int32Array` view of it (`workerData.addonCrashFlag`). The worker raises it
// with `Atomics.store` right after the loader flag, with no instance needed, so a
// worker that fails while it loads raises it too; its setup (the runtime require
// through `new MessageHandler`) is wrapped in a try/catch that raises both flags
// and rethrows. Without the view it falls back to the `napi_wasm_thread_crashed`
// export, which needs an instance.
// CurrentThread host timers: a sleep arms a referenced `setTimeout` that only
// the normal teardown releases, so a crash disposal would leave the process
// alive until it fires. The loader hands the async-runtime host installer a
// view of the binding (`__trackCurrentThreadHostTimers`) that records each
// timer's cancel, and both crash paths (the crash disposal and the exit
// listener) call `__releaseCurrentThreadHostTimers()`, which is plain JS.
// See internal-docs/async-runtime/implementation.md (section 7, Loaders).
const WASI_THREAD_CRASH_LATCH_LOADER_SIGNATURES = [
  'const __wasiThreadCrashFlag = new Int32Array(new SharedArrayBuffer(4))',
  'function __hasWasiThreadCrashed() {',
  'crashFlag: __wasiThreadCrashFlag,',
  `function __disposeWasiBindingAtExit() {
  __wasiExitListenerRegistered = false
  if (__hasWasiThreadCrashed()) {
`,
  `function __disposeWasiBinding() {
  if (__wasiDisposePromise) {
    return __wasiDisposePromise
  }
  if (!__wasiDisposed && __hasWasiThreadCrashed()) {
    return __disposeWasiBindingAfterThreadCrash()
  }
`,
  'function __disposeWasiBindingAfterThreadCrash() {',
  'function __releaseEmnapiWaitingRequestHandle() {',
  `    const refHandle = refCounter && refCounter.refHandle
    if (refHandle && typeof refHandle.unref === 'function') {
      refHandle.unref()
    }
`,
  `  __releaseEmnapiWaitingRequestHandle()
  __releaseCurrentThreadHostTimers()
  let workerResult
  try {
    workerResult = __terminateWasiWorkers()
`,
  // In-flight disposal: every poll turn and step boundary stops the chain after
  // a crash, and the barrier's finish is skipped instead of joining dead work.
  'function __abortWasiDisposalIfThreadCrashed() {',
  'function __settleWasiDisposalAfterThreadCrash(resolve, reject) {',
  `      await __yieldWasmRuntimePollTurn(pace)
      __abortWasiDisposalIfThreadCrashed()
`,
  '})().then(finishCleanupUnlessCrashed, finishCleanupUnlessCrashed)',
  `          __scheduleTimer(resolve, __WASI_ASYNC_WORK_POLL_INTERVAL_MS)
        })
        __abortWasiDisposalIfThreadCrashed()
`,
  // Initialization rollback: same latch, through its own wrapper.
  'let __wasiInitializationRollbackActive = false',
  'function __runWasiInitializationRollbackSteps() {',
  'function __rollbackWasiInitializationAfterThreadCrash() {',
  // Shared crash report: the worker's error and threadId for the rejection.
  'const __wasiThreadCrashReport = new SharedArrayBuffer(4096)',
  'crashReport: __wasiThreadCrashReport,',
  'function __readWasiThreadCrashReport() {',
  'function __getWasiThreadCrashError() {',
  '__recordWasiThreadCrashError(error, worker.threadId)',
  // Addon crash flag bridge: read the address before registration, validate it,
  // and hand the view to every pool worker.
  'function __captureWasiAddonCrashFlag(instance) {',
  'const getAddress = instance.exports.napi_wasm_thread_crash_flag_address',
  `    if (address === 0 || address % 4 !== 0 || address + 4 > buffer.byteLength) {
      return
    }
    __wasiAddonCrashFlag = new Int32Array(buffer, address, 1)
`,
  `    beforeInit({ instance }) {
      __napiInstance = instance
      __captureWasiAddonCrashFlag(instance)
      for (const name of Object.keys(instance.exports)) {
`,
  'addonCrashFlag: __wasiAddonCrashFlag,',
  // CurrentThread host timers: tracked through the binding view, released
  // without entering wasm on both crash paths (the crash disposal above and
  // the exit listener below).
  'function __trackCurrentThreadHostTimers(binding) {',
  '__trackCurrentThreadHostTimers(__napiModule.exports)',
  'function __releaseCurrentThreadHostTimers() {',
  `    // workers and leave.
    __releaseCurrentThreadHostTimers()
    try {
      void Promise.resolve(__terminateWasiWorkers()).catch(() => {})
    } catch {}
    return
  }
`,
] as const;
const WASI_THREAD_CRASH_LATCH_WORKER_SIGNATURES = [
  // A failed setup (before the crash hook exists) raises both flags and rethrows.
  `} catch (error) {
  if (workerData && workerData.crashFlag instanceof Int32Array) {
    __raiseWasiThreadCrashFlags(error)
  }
  throw error
}
`,
  `if (workerData && workerData.crashFlag instanceof Int32Array) {
  const __beforeReportError = handler.beforeReportError
  handler.beforeReportError = function (...args) {
    if (!__raiseWasiThreadCrashFlags(args[0])) {
`,
  'const __napiThreadCrashed = this.instance?.exports?.napi_wasm_thread_crashed',
  'function __raiseWasiThreadCrashFlags(error) {',
  // Report first, then the loader flag, then the addon flag.
  `    __writeCrashReport(workerData.crashReport, error)
  } catch {}
  try {
    Atomics.store(workerData.crashFlag, 0, 1)
  } catch {}
  const addonCrashFlag = workerData.addonCrashFlag
`,
  'Atomics.store(addonCrashFlag, 0, 1)',
  'function __writeCrashReport(report, error) {',
] as const;
// Pool worker preload, threaded Node flavor only (`rolldown-binding.wasi.cjs`).
// The cli (napi-rs feat/wasi-thread-pool-preload) emits it: after the load the
// loader reads the addon's `napi_wasm_runtime_pool_workers` export (from
// napi-async-runtime) and keeps that many Workers loading in emnapi's idle pool,
// and it wraps `configureAsyncRuntime` so a later configure matches the pool to
// the new count. Rolldown only checks that the seam is still there.
// See internal-docs/async-runtime/implementation.md (section 13, "Pool worker preload").
const WASI_THREAD_POOL_PRELOAD_LOADER_SIGNATURES = [
  'function __reconcileWasiThreadPool() {',
  // Where the count comes from.
  'const read = __napiInstance?.exports?.napi_wasm_runtime_pool_workers',
  // The pool the Workers go into, and a thread spawn takes them from.
  'reuseWorker: true,',
  'function __getWasiThreadManager() {',
  "const __wasiThreadPoolReconcileSymbol = Symbol.for('napi.rs.wasi.reconcileThreadPool')",
  '  __publishWasiThreadPoolReconcile(__napiModule.exports)\n',
  // The configure wrap. The export tail reads the wrapped function back off the
  // binding, so the ESM namespace in dist sees it too.
  'function __wrapWasiConfigureAsyncRuntime(binding) {',
  '  __wrapWasiConfigureAsyncRuntime(__napiModule.exports)\n',
  'module.exports.configureAsyncRuntime = __napiModule.exports.configureAsyncRuntime\n',
] as const;
// The preload call: once, after the load try/catch, before the export tail.
const WASI_THREAD_POOL_PRELOAD_CALL = `try {
  __reconcileWasiThreadPool()
} catch {}
`;
const WASI_LOAD_FAILURE_RETHROW = `  throw rollback.error
}
`;
const WASI_CJS_EXPORT_TAIL = 'module.exports = __napiModule.exports\n';

/**
 * Assert the upstream (`@napi-rs/cli` >= 3.10.0) context lifecycle seams and
 * return the loader unchanged.
 *
 * Nothing here rewrites the generated source any more: every seam below is
 * emitted by the cli itself. The assertions make a cli bump that drops or
 * reshapes any teardown seam fail the build loudly instead of silently
 * regressing teardown.
 */
export function assertWasiBindingContextLifecycle(source: string): void {
  const cjsDirectImportCount = countOccurrences(source, WASI_CJS_CREATE_CONTEXT_IMPORT);
  const esmDirectImportCount = countOccurrences(source, WASI_ESM_CREATE_CONTEXT_IMPORT);
  if (cjsDirectImportCount + esmDirectImportCount !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for context import: expected one direct @emnapi/runtime createContext import, found ${cjsDirectImportCount + esmDirectImportCount}`,
    );
  }

  for (const signature of WASI_DISPOSAL_CHAIN_SIGNATURES) {
    assertExactlyOne(source, signature, 'WASI disposal chain helper');
  }
  for (const wait of WASI_ASYNC_TEARDOWN_WAITS) {
    assertExactlyOne(source, wait.snippet, wait.label);
  }
  assertExactlyOne(source, WASI_CONTEXT_SUPPRESS_DESTROY, 'WASI context auto-destroy suppression');
  assertExactlyOne(
    source,
    WASI_CONTEXT_PREPARE_CLEANUP_FLAG,
    'WASI context cleanup preparation state',
  );
  // The only raw context destroy lives inside __destroyEmnapiContext, directly
  // behind the settlement barrier.
  assertExactlyOne(source, '__emnapiContext.destroy()', 'WASI context destroy operation');
  assertExactlyOne(
    source,
    WASI_CONTEXT_DESTROY_SETTLEMENT,
    'WASI context destroy settlement barrier',
  );
  assertExactlyOne(source, WASI_CONTEXT_DESTROY_WRAP_HELPER, 'WASI context destroy wrapper');
  assertExactlyOneNormalized(
    source,
    WASI_CONTEXT_DESTROY_WRAP_WIRING,
    'WASI context destroy settlement wiring',
  );
  assertExactlyOne(source, WASI_DISPOSE_PUBLICATION, 'WASI dispose symbol publication');
  const isCommonJs = cjsDirectImportCount === 1;
  const exitListenerCount = countOccurrences(source, WASI_EXIT_LISTENER_HELPER);
  if (isCommonJs && exitListenerCount !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for exit-time teardown: expected one exit listener helper, found ${exitListenerCount}`,
    );
  }
}

/**
 * Assert the worker-crash latch in the threaded Node loader and its pool
 * worker. The threadless and browser loaders have no latch (no pool workers
 * or no exit teardown), so only these two files are checked.
 */
export function assertWasiThreadCrashLatch(loaderSource: string, workerSource: string): void {
  for (const signature of WASI_THREAD_CRASH_LATCH_LOADER_SIGNATURES) {
    assertExactlyOne(loaderSource, signature, 'WASI thread crash latch (loader)');
  }
  for (const signature of WASI_THREAD_CRASH_LATCH_WORKER_SIGNATURES) {
    assertExactlyOne(workerSource, signature, 'WASI thread crash latch (worker)');
  }
}

/**
 * Assert the pool worker preload in the threaded Node loader: the cli emits it,
 * rolldown keeps no copy of its own. A cli bump that drops or moves it fails
 * the build instead of silently putting the Worker boot back on the first
 * build.
 */
export function assertWasiThreadPoolPreload(loaderSource: string): void {
  for (const signature of WASI_THREAD_POOL_PRELOAD_LOADER_SIGNATURES) {
    assertExactlyOne(loaderSource, signature, 'WASI thread pool preload');
  }
  assertExactlyOne(loaderSource, WASI_THREAD_POOL_PRELOAD_CALL, 'WASI thread pool preload call');
  assertExactlyOne(loaderSource, WASI_LOAD_FAILURE_RETHROW, 'WASI load failure rethrow');
  assertExactlyOne(loaderSource, WASI_CJS_EXPORT_TAIL, 'WASI CommonJS export tail');
  const rethrow = loaderSource.indexOf(WASI_LOAD_FAILURE_RETHROW);
  const call = loaderSource.indexOf(WASI_THREAD_POOL_PRELOAD_CALL);
  const tail = loaderSource.indexOf(WASI_CJS_EXPORT_TAIL);
  if (call < rethrow || tail < call) {
    throw new Error(
      'Unexpected NAPI-RS WASI loader template for WASI thread pool preload: expected the preload call between the load try/catch and the CommonJS export tail',
    );
  }
}

export function assertAsyncRuntimeHostExports(
  source: string,
  moduleFormat: BindingLoaderModuleFormat,
): void {
  const missing = ASYNC_RUNTIME_HOST_EXPORTS.filter((name) => {
    const assignment =
      moduleFormat === 'commonjs' ? `module.exports.${name} =` : `export const ${name} =`;
    return !source.includes(assignment);
  });
  if (missing.length > 0) {
    throw new Error(
      `Generated ${moduleFormat} binding loader is missing async-runtime host exports: ${missing.join(', ')}`,
    );
  }
}

function normalizeWhitespace(source: string): string {
  return source.replace(/\s+/g, ' ');
}

function countOccurrences(source: string, search: string): number {
  return source.split(search).length - 1;
}

function assertExactlyOne(source: string, search: string, label: string): void {
  const count = countOccurrences(source, search);
  if (count !== 1) {
    throw new Error(
      `Unexpected NAPI-RS loader template for ${label}: expected 1 anchor, found ${count}`,
    );
  }
}

function assertExactlyOneNormalized(source: string, search: string, label: string): void {
  assertExactlyOne(normalizeWhitespace(source), normalizeWhitespace(search), label);
}
