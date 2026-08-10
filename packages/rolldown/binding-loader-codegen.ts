export const LOADED_BINDING_TARGET_EXPORT = '__rolldownBindingTarget';
export const EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT = 4;
export const EMNAPI_ASYNC_WORK_POOL_SIZE_MAX = 1024;
export const ASYNC_RUNTIME_HOST_EXPORTS = [
  'getCurrentThreadTaskHostContractVersion',
  'isCurrentThreadHostRegistrationActive',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
  'reserveCurrentThreadHostRegistration',
  'unregisterCurrentThreadTaskHost',
  'unregisterTimerHost',
] as const;

type LoadedBindingTarget = 'native' | 'wasi' | 'wasi-threads';
export type WasiBindingTarget = Exclude<LoadedBindingTarget, 'native'>;
export type BindingLoaderModuleFormat = 'commonjs' | 'esm';

const WASI_TARGET = 'wasm32-wasip1';
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
const NATIVE_BINDING_ANCHOR = 'let nativeBinding = null\n';
const WASI_BINDING_ASSIGNMENT = 'nativeBinding = wasiBinding';
const NATIVE_BINDING_EXPORT_ANCHOR = 'module.exports = nativeBinding\n';
const WASI_CJS_EXPORT_ANCHOR = 'module.exports = __napiModule.exports\n';
const WASI_ESM_EXPORT_ANCHOR = 'export default __napiModule.exports\n';
const WASI_CJS_CREATE_CONTEXT_IMPORT =
  "const { createContext: __emnapiCreateContext } = require('@emnapi/runtime')\n";
const WASI_ESM_CREATE_CONTEXT_IMPORT =
  "import { createContext as __emnapiCreateContext } from '@emnapi/runtime'\n";
const WASI_CONTEXT_CREATION = '__emnapiContext = __emnapiCreateContext({ autoDestroy: false })';
const WASI_CONTEXT_SUPPRESS_DESTROY = '__emnapiContext.suppressDestroy()';
const WASI_CONTEXT_PREPARE_CLEANUP_FLAG = 'let __emnapiWasmEnvCleanupPrepared = false\n';
const WASI_PREPARE_CLEANUP_HELPER = 'function __prepareWasmEnvCleanup() {';
const WASI_CONTEXT_DESTROY_WRAP_HELPER = 'function __wrapEmnapiContextDestroyForSettlement(';
// A raw destroy call on the emnapi context (bypassing the published dispose
// symbol and the loader's own teardown) must still settle pending napi async
// work: the wasm-side cleanup preparation cancels the runtime's tasks while
// the environment can still call into JavaScript, so their deferreds reject
// with the runtime's cancellation error instead of panicking on a dead
// threadsafe function. The upstream template only guards its own destroy
// paths, so the loader wraps the context it creates.
const WASI_CONTEXT_DESTROY_WRAP = `function __wrapEmnapiContextDestroyForSettlement(context) {
  // oxlint-disable-next-line typescript/unbound-method -- invoked with the wrapper receiver below
  const __contextDestroy = context.destroy
  context.destroy = function () {
    __prepareWasmEnvCleanup()
    return Reflect.apply(__contextDestroy, this, arguments)
  }
  return context
}

`;
// The pre-destroy settlement barrier: __destroyEmnapiContext must run the
// wasm-side cleanup preparation before the context destroy so pending napi
// async work settles instead of being discarded by the TSFN cleanup hook.
const WASI_CONTEXT_DESTROY_SETTLEMENT = `  __prepareWasmEnvCleanup()
  const result = __emnapiContext.destroy()
`;
// The disposal chain runs prepare -> drain -> destroy -> worker termination
// and publishes Symbol.for('napi.rs.wasi.dispose') on the binding exports.
const WASI_DISPOSAL_CHAIN_SIGNATURES = [
  'function __prepareWasmEnvCleanup() {',
  'function __drainWasmEnvCleanup() {',
  'function __destroyEmnapiContext() {',
  'function __terminateWasiWorkers() {',
  'function __startWasiDisposal() {',
  'function __disposeWasiBinding() {',
  'function __publishWasiDispose(exports) {',
  'function __rollbackWasiInitialization() {',
] as const;
const WASI_DISPOSE_PUBLICATION = '__publishWasiDispose(__napiModule.exports)';
const WASI_EXIT_LISTENER_HELPER = 'function __registerWasiExitListener() {';
const WASI_NODE_HELPER_ANCHOR = 'const __rootDir = __nodePath.parse(process.cwd()).root\n';
const WASI_NODE_ENV_ASSIGNMENT = 'env: process.env,';
const WASI_NODE_WORKER_HELPER_SIGNATURES = [
  'function __getWasiWorkerExecArgv() {',
  'function __isInvalidWasiWorkerExecArgv(errorMessage, argument) {',
  'function __removeInvalidWasiWorkerExecArgv(execArgv, error) {',
  'function __createWasiWorker(filename) {',
] as const;
const WASI_NODE_WORKER_CONSTRUCTION =
  "const worker = __createWasiWorker(__nodePath.join(__dirname, 'wasi-worker.mjs'))";
const WASI_NODE_ASYNC_WORK_POOL_SIZE = `    asyncWorkPoolSize: (function() {
      const threadsSizeFromEnv = Number(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE ?? process.env.UV_THREADPOOL_SIZE)
      // NaN > 0 is false
      if (threadsSizeFromEnv > 0) {
        return threadsSizeFromEnv
      } else {
        return 4
      }
    })(),`;
const WASI_CJS_TARGET_PATTERN = new RegExp(
  `module\\.exports\\.${LOADED_BINDING_TARGET_EXPORT}\\s*=\\s*[^\\r\\n]+`,
  'g',
);
const WASI_ESM_TARGET_PATTERN = new RegExp(
  `export const ${LOADED_BINDING_TARGET_EXPORT}\\s*=\\s*[^\\r\\n]+`,
  'g',
);

export function resolveWasiBindingTarget(target: unknown): WasiBindingTarget {
  if (target === WASI_TARGET) return 'wasi';
  if (target === WASI_THREADS_TARGET || target === undefined) return 'wasi-threads';
  if (typeof target === 'string' && !target.startsWith('wasm')) return 'wasi-threads';
  throw new Error(`Unsupported WASI binding target: ${String(target)}`);
}

/**
 * Normalize the napi-rs pool environment before emnapi receives it.
 *
 * The upstream loader accepts any positive Number, after which emnapi applies
 * ToInt32 and a 1024 cap. Canonicalizing first keeps the actual pool and the
 * value visible to the WASI guest identical, including scientific and hex
 * input forms accepted by Number().
 */
export function normalizeEmnapiAsyncWorkPoolSize(value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || numeric <= 0) {
    return EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT;
  }
  const integer = Math.trunc(numeric);
  return integer > 0
    ? Math.min(integer, EMNAPI_ASYNC_WORK_POOL_SIZE_MAX)
    : EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT;
}

export function patchNativeBindingLoader(source: string): string {
  if (source.includes(`module.exports.${LOADED_BINDING_TARGET_EXPORT} = loadedBindingTarget`)) {
    return source;
  }

  source = replaceExactly(
    source,
    NATIVE_BINDING_ANCHOR,
    `${NATIVE_BINDING_ANCHOR}let loadedBindingTarget = 'native'\n`,
    1,
    'native binding declaration',
  );
  const wasiBindingAssignmentCount = source.split(WASI_BINDING_ASSIGNMENT).length - 1;
  if (wasiBindingAssignmentCount < 2 || wasiBindingAssignmentCount % 2 !== 0) {
    throw new Error(
      `Unexpected NAPI-RS loader template for WASI binding assignments: expected a positive pair count, found ${wasiBindingAssignmentCount}`,
    );
  }
  source = source.replaceAll(
    WASI_BINDING_ASSIGNMENT,
    `${WASI_BINDING_ASSIGNMENT}
      loadedBindingTarget =
        wasiBinding.${LOADED_BINDING_TARGET_EXPORT} === 'wasi' ? 'wasi' : 'wasi-threads'`,
  );
  return replaceExactly(
    source,
    NATIVE_BINDING_EXPORT_ANCHOR,
    `${NATIVE_BINDING_EXPORT_ANCHOR}module.exports.${LOADED_BINDING_TARGET_EXPORT} = loadedBindingTarget\n`,
    1,
    'native binding export',
  );
}

export function patchWasiBindingLoader(source: string, target: WasiBindingTarget): string {
  const cjsExport = `module.exports.${LOADED_BINDING_TARGET_EXPORT} = '${target}'`;
  const esmExport = `export const ${LOADED_BINDING_TARGET_EXPORT} = '${target}'`;
  const cjsTargets = source.match(WASI_CJS_TARGET_PATTERN) ?? [];
  const esmTargets = source.match(WASI_ESM_TARGET_PATTERN) ?? [];
  const targetCount = cjsTargets.length + esmTargets.length;

  if (targetCount > 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template: expected at most one binding target export, found ${targetCount}`,
    );
  }
  if (cjsTargets.length === 1) {
    return source.replace(cjsTargets[0], cjsExport);
  }
  if (esmTargets.length === 1) {
    return source.replace(esmTargets[0], esmExport);
  }
  if (source.includes(WASI_CJS_EXPORT_ANCHOR)) {
    return replaceExactly(
      source,
      WASI_CJS_EXPORT_ANCHOR,
      `${WASI_CJS_EXPORT_ANCHOR}${cjsExport}\n`,
      1,
      'WASI CommonJS binding export',
    );
  }
  if (source.includes(WASI_ESM_EXPORT_ANCHOR)) {
    return replaceExactly(
      source,
      WASI_ESM_EXPORT_ANCHOR,
      `${WASI_ESM_EXPORT_ANCHOR}${esmExport}\n`,
      1,
      'WASI ESM binding export',
    );
  }
  throw new Error('Unexpected NAPI-RS WASI loader template: no module export anchor');
}

/**
 * Verify the generated loader carries the upstream (>= 3.8.4) context
 * lifecycle contract this repo relies on, then harden its raw destroy path.
 *
 * @napi-rs/cli 3.8.4 ships the full lifecycle that earlier rolldown patch
 * layers injected: an isolated non-auto-destroying context, the
 * `napi_prepare_wasm_env_cleanup` settlement barrier guarded by
 * `__emnapiWasmEnvCleanupPrepared`, the macrotask-yield settlement drain
 * (`__drainWasmEnvCleanup` polling `napi_wasm_env_cleanup_pending`, rejecting
 * retryably with `ERR_NAPI_WASI_CLEANUP_PENDING`), a thenable-aware disposal
 * chain published as `Symbol.for('napi.rs.wasi.dispose')`, and an
 * initialization-failure rollback. This assertion set pins those seams so a
 * future CLI bump that drops or reshapes any of them fails the build loudly
 * instead of silently regressing teardown.
 *
 * The one remaining rolldown addition is the raw-destroy settlement wrapper:
 * upstream only runs the settlement barrier on its own destroy paths, while
 * rolldown's lifecycle suite pins that a direct `context.destroy()` call also
 * settles pending work (see `WASI_CONTEXT_DESTROY_WRAP`).
 */
export function patchWasiBindingContextLifecycle(source: string): string {
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
  assertExactlyOne(source, WASI_DISPOSE_PUBLICATION, 'WASI dispose symbol publication');
  const isCommonJs = cjsDirectImportCount === 1;
  const exitListenerCount = countOccurrences(source, WASI_EXIT_LISTENER_HELPER);
  if (isCommonJs && exitListenerCount !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for exit-time teardown: expected one exit listener helper, found ${exitListenerCount}`,
    );
  }

  // A wasi-target build only regenerates its own flavor's loaders, so the
  // other flavor's files arrive here already carrying the wrapper: verify it
  // and return them unchanged.
  const wrappedCreation =
    '__emnapiContext = __wrapEmnapiContextDestroyForSettlement(__emnapiCreateContext({ autoDestroy: false }))';
  if (countOccurrences(source, WASI_CONTEXT_DESTROY_WRAP_HELPER) > 0) {
    assertExactlyOne(source, WASI_CONTEXT_DESTROY_WRAP, 'WASI context destroy settlement wrapper');
    assertExactlyOne(source, wrappedCreation, 'WASI context destroy settlement wiring');
    return source;
  }

  assertExactlyOne(source, WASI_CONTEXT_CREATION, 'WASI isolated context creation');
  source = replaceExactly(
    source,
    WASI_PREPARE_CLEANUP_HELPER,
    `${WASI_CONTEXT_DESTROY_WRAP}${WASI_PREPARE_CLEANUP_HELPER}`,
    1,
    'WASI context destroy settlement wrapper',
  );
  return replaceExactly(
    source,
    WASI_CONTEXT_CREATION,
    wrappedCreation,
    1,
    'WASI context destroy settlement wiring',
  );
}

/**
 * Verify the generated Node WASI loader spawns its threads through the
 * hardened worker factory and return it unchanged.
 *
 * @napi-rs/cli 3.8.4 ships the exec-argv sanitizing worker helpers this repo
 * previously injected (`__getWasiWorkerExecArgv` filtering eval/print
 * arguments, `__removeInvalidWasiWorkerExecArgv` retrying on
 * ERR_WORKER_INVALID_EXEC_ARGV, and the `__createWasiWorker` factory).
 */
export function patchWasiNodeWorkerExecArgv(source: string): string {
  for (const signature of WASI_NODE_WORKER_HELPER_SIGNATURES) {
    assertExactlyOne(source, signature, 'WASI worker execArgv helper');
  }
  assertExactlyOne(source, WASI_NODE_WORKER_CONSTRUCTION, 'WASI worker construction');
  return source;
}

export function patchWasiNodeAsyncWorkPoolSize(source: string): string {
  if (source.includes('const __rolldownAsyncWorkPoolSize =')) {
    return source;
  }

  const normalization = `function __normalizeRolldownAsyncWorkPoolSize(value) {
  const numeric = Number(value)
  if (!Number.isFinite(numeric) || numeric <= 0) {
    return ${EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT}
  }
  const integer = Math.trunc(numeric)
  return integer > 0
    ? Math.min(integer, ${EMNAPI_ASYNC_WORK_POOL_SIZE_MAX})
    : ${EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT}
}

const __rolldownAsyncWorkPoolSize = __normalizeRolldownAsyncWorkPoolSize(
  process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE ?? process.env.UV_THREADPOOL_SIZE,
)
const __rolldownWasiEnv = {
  ...process.env,
  NAPI_RS_ASYNC_WORK_POOL_SIZE: String(__rolldownAsyncWorkPoolSize),
}

`;
  source = replaceExactly(
    source,
    WASI_NODE_HELPER_ANCHOR,
    normalization + WASI_NODE_HELPER_ANCHOR,
    1,
    'WASI async-work-pool helper',
  );
  source = replaceExactly(
    source,
    WASI_NODE_ASYNC_WORK_POOL_SIZE,
    '    asyncWorkPoolSize: __rolldownAsyncWorkPoolSize,',
    1,
    'WASI async-work-pool option',
  );
  // One environment on the WASI instance, one inside __createWasiWorker.
  const environmentCount = countOccurrences(source, WASI_NODE_ENV_ASSIGNMENT);
  if (environmentCount !== 2) {
    throw new Error(
      `Unexpected NAPI-RS loader template for WASI runtime and worker environments: expected 2 anchors, found ${environmentCount}`,
    );
  }
  return source.replaceAll(WASI_NODE_ENV_ASSIGNMENT, 'env: __rolldownWasiEnv,');
}

/**
 * Verify the generated browser WASI loader tears its context down through the
 * thenable-aware disposal chain and return it unchanged.
 *
 * @napi-rs/cli 3.8.4 routes every browser context destroy through
 * `__destroyEmnapiContext()` behind `__isThenable` checks, so a synchronous
 * emnapi destroy and a promise-returning one settle identically; there is no
 * bare `await __emnapiContext.destroy()` left to normalize.
 */
export function patchWasiBrowserContextDestroyAwait(source: string): string {
  assertExactlyOne(
    source,
    `  const destroyResult = __destroyEmnapiContext()
  if (__isThenable(destroyResult)) {
`,
    'WASI browser thenable-aware context destroy',
  );
  if (countOccurrences(source, 'await __emnapiContext.destroy()') !== 0) {
    throw new Error(
      'Unexpected NAPI-RS WASI browser cleanup template: found a bare context destroy await outside the disposal chain',
    );
  }
  return source;
}

/**
 * Verify the generated browser WASI loader collects worker termination
 * results uniformly and return it unchanged.
 *
 * @napi-rs/cli 3.8.4's `__terminateWasiWorkers` wraps thenable termination
 * results in `Promise.resolve` and funnels synchronous failures into the same
 * cleanup error aggregation, so mixed settled/unsettled termination entries
 * can no longer race the disposal.
 */
export function patchWasiBrowserWorkerTerminationAwait(source: string): string {
  assertExactlyOne(
    source,
    'function __terminateWasiWorkers() {',
    'WASI browser worker termination',
  );
  assertExactlyOne(
    source,
    `    if (__isThenable(result)) {
      pending.push(
        Promise.resolve(result).then(
`,
    'WASI browser thenable-aware worker termination',
  );
  return source;
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

function replaceExactly(
  source: string,
  search: string,
  replacement: string,
  expectedCount: number,
  label: string,
): string {
  const count = countOccurrences(source, search);
  if (count !== expectedCount) {
    throw new Error(
      `Unexpected NAPI-RS loader template for ${label}: expected ${expectedCount} anchors, found ${count}`,
    );
  }
  return source.replaceAll(search, replacement);
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
