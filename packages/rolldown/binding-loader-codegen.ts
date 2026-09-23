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
