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
const WASI_CJS_CONTEXT_IMPORT = '  getDefaultContext: __emnapiGetDefaultContext,\n';
const WASI_ESM_CONTEXT_IMPORT = '  getDefaultContext as __emnapiGetDefaultContext,\n';
const WASI_CJS_WASM_RUNTIME_CREATE_CONTEXT_IMPORT = '  createContext: __emnapiCreateContext,\n';
const WASI_ESM_WASM_RUNTIME_CREATE_CONTEXT_IMPORT = '  createContext as __emnapiCreateContext,\n';
const WASI_CJS_RUNTIME_IMPORT_ANCHOR = "} = require('@napi-rs/wasm-runtime')\n";
const WASI_ESM_RUNTIME_IMPORT_ANCHOR = "} from '@napi-rs/wasm-runtime'\n";
const WASI_CJS_CREATE_CONTEXT_IMPORT =
  "const { createContext: __emnapiCreateContext } = require('@emnapi/runtime')\n";
const WASI_ESM_CREATE_CONTEXT_IMPORT =
  "import { createContext as __emnapiCreateContext } from '@emnapi/runtime'\n";
const WASI_DEFAULT_CONTEXT_CREATION = 'const __emnapiContext = __emnapiGetDefaultContext()\n';
const WASI_ISOLATED_CONTEXT_CREATION = 'const __emnapiContext = __emnapiCreateContext()\n';
const WASI_BEFORE_INIT_ANCHOR = '  beforeInit({ instance }) {\n';
const WASI_CONTEXT_DESTROY_CALL = '    __wrapEmnapiContextDestroy(instance)\n';
const WASI_NODE_HELPER_ANCHOR = 'const __rootDir = __nodePath.parse(process.cwd()).root\n';
const WASI_NODE_ENV_ASSIGNMENT = 'env: process.env,';
const WASI_NODE_WORKER_CONSTRUCTION = `    const worker = new Worker(__nodePath.join(__dirname, 'wasi-worker.mjs'), {
      env: process.env,
    })`;
const WASI_NODE_ASYNC_WORK_POOL_SIZE = `  asyncWorkPoolSize: (function() {
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
  source = replaceExactly(
    source,
    WASI_BINDING_ASSIGNMENT,
    `${WASI_BINDING_ASSIGNMENT}
      loadedBindingTarget =
        wasiBinding.${LOADED_BINDING_TARGET_EXPORT} === 'wasi' ? 'wasi' : 'wasi-threads'`,
    2,
    'WASI binding assignment',
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

export function patchWasiBindingContextLifecycle(source: string): string {
  const cjsDirectImportCount = countOccurrences(source, WASI_CJS_CREATE_CONTEXT_IMPORT);
  const esmDirectImportCount = countOccurrences(source, WASI_ESM_CREATE_CONTEXT_IMPORT);
  const cjsWasmRuntimeImportCount =
    countOccurrences(source, WASI_CJS_CONTEXT_IMPORT) +
    countOccurrences(source, WASI_CJS_WASM_RUNTIME_CREATE_CONTEXT_IMPORT);
  const esmWasmRuntimeImportCount =
    countOccurrences(source, WASI_ESM_CONTEXT_IMPORT) +
    countOccurrences(source, WASI_ESM_WASM_RUNTIME_CREATE_CONTEXT_IMPORT);
  const directImportCount = cjsDirectImportCount + esmDirectImportCount;
  const wasmRuntimeImportCount = cjsWasmRuntimeImportCount + esmWasmRuntimeImportCount;

  if (directImportCount > 1 || wasmRuntimeImportCount > 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for context import: expected one anchor, found ${directImportCount + wasmRuntimeImportCount}`,
    );
  }
  if (directImportCount === 1 && wasmRuntimeImportCount !== 0) {
    throw new Error('Unexpected NAPI-RS WASI loader template: duplicate context imports');
  }
  if (directImportCount === 0 && wasmRuntimeImportCount !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for context import: expected one anchor, found ${wasmRuntimeImportCount}`,
    );
  }

  if (cjsWasmRuntimeImportCount === 1) {
    source = source
      .replace(WASI_CJS_CONTEXT_IMPORT, '')
      .replace(WASI_CJS_WASM_RUNTIME_CREATE_CONTEXT_IMPORT, '');
    source = replaceExactly(
      source,
      WASI_CJS_RUNTIME_IMPORT_ANCHOR,
      `${WASI_CJS_RUNTIME_IMPORT_ANCHOR}${WASI_CJS_CREATE_CONTEXT_IMPORT}`,
      1,
      'WASI CommonJS emnapi context import',
    );
  } else if (esmWasmRuntimeImportCount === 1) {
    source = source
      .replace(WASI_ESM_CONTEXT_IMPORT, '')
      .replace(WASI_ESM_WASM_RUNTIME_CREATE_CONTEXT_IMPORT, '');
    source = replaceExactly(
      source,
      WASI_ESM_RUNTIME_IMPORT_ANCHOR,
      `${WASI_ESM_RUNTIME_IMPORT_ANCHOR}${WASI_ESM_CREATE_CONTEXT_IMPORT}`,
      1,
      'WASI ESM emnapi context import',
    );
  }

  const contextLifecycle = `${WASI_ISOLATED_CONTEXT_CREATION}
let __emnapiContextDestroyWrapped = false
let __emnapiWasmEnvCleanupPrepared = false

function __wrapEmnapiContextDestroy(instance) {
  if (__emnapiContextDestroyWrapped) {
    return
  }
  // oxlint-disable-next-line typescript/unbound-method -- invoked with the wrapper receiver below
  const __destroyEmnapiContext = __emnapiContext.destroy
  __emnapiContext.destroy = function() {
    if (!__emnapiWasmEnvCleanupPrepared) {
      const __prepareWasmEnvCleanup =
        instance.exports.napi_prepare_wasm_env_cleanup
      if (typeof __prepareWasmEnvCleanup === 'function') {
        __prepareWasmEnvCleanup()
      }
      __emnapiWasmEnvCleanupPrepared = true
    }
    return Reflect.apply(__destroyEmnapiContext, this, arguments)
  }
  __emnapiContextDestroyWrapped = true
}
`;
  if (source.includes(WASI_DEFAULT_CONTEXT_CREATION)) {
    source = replaceExactly(
      source,
      WASI_DEFAULT_CONTEXT_CREATION,
      contextLifecycle,
      1,
      'WASI isolated context creation',
    );
  } else if (
    source.includes(WASI_ISOLATED_CONTEXT_CREATION) &&
    !source.includes('function __wrapEmnapiContextDestroy(instance)')
  ) {
    source = replaceExactly(
      source,
      WASI_ISOLATED_CONTEXT_CREATION,
      contextLifecycle,
      1,
      'WASI context destroy wrapper',
    );
  } else if (!source.includes('function __wrapEmnapiContextDestroy(instance)')) {
    throw new Error('Unexpected NAPI-RS WASI loader template: no context creation anchor');
  }

  const destroyCallCount = countOccurrences(source, WASI_CONTEXT_DESTROY_CALL);
  if (destroyCallCount === 0) {
    return replaceExactly(
      source,
      WASI_BEFORE_INIT_ANCHOR,
      `${WASI_BEFORE_INIT_ANCHOR}${WASI_CONTEXT_DESTROY_CALL}`,
      1,
      'WASI context destroy preparation',
    );
  }
  if (destroyCallCount !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI loader template for context destroy preparation: expected one call, found ${destroyCallCount}`,
    );
  }
  return source;
}

export function patchWasiNodeWorkerExecArgv(source: string): string {
  if (source.includes('function __createWasiWorker(filename)')) {
    return source;
  }

  const workerHelpers = `function __getWasiWorkerExecArgv() {
  const __workerExecArgv = []
  for (let __index = 0; __index < process.execArgv.length; __index += 1) {
    const __arg = process.execArgv[__index]
    if (
      __arg === '--input-type' ||
      __arg === '--eval' ||
      __arg === '-e' ||
      __arg === '--print' ||
      __arg === '-p'
    ) {
      __index += 1
      continue
    }
    if (
      __arg.startsWith('--input-type=') ||
      __arg.startsWith('--eval=') ||
      __arg.startsWith('--print=')
    ) {
      continue
    }
    __workerExecArgv.push(__arg)
  }
  return __workerExecArgv
}

function __isInvalidWasiWorkerExecArgv(errorMessage, argument) {
  const __equalsIndex = argument.indexOf('=')
  const __argumentName =
    __equalsIndex === -1 ? argument : argument.slice(0, __equalsIndex)
  return (
    errorMessage.includes(': ' + __argumentName + ',') ||
    errorMessage.includes(': ' + __argumentName + '=') ||
    errorMessage.endsWith(': ' + __argumentName) ||
    errorMessage.includes(', ' + __argumentName + ',') ||
    errorMessage.includes(', ' + __argumentName + '=') ||
    errorMessage.endsWith(', ' + __argumentName)
  )
}

function __removeInvalidWasiWorkerExecArgv(execArgv, error) {
  if (typeof error.message !== 'string') {
    return
  }
  const __workerExecArgv = []
  let __removed = false
  for (let __index = 0; __index < execArgv.length; __index += 1) {
    const __arg = execArgv[__index]
    if (
      __arg.startsWith('-') &&
      __isInvalidWasiWorkerExecArgv(error.message, __arg)
    ) {
      __removed = true
      if (
        !__arg.includes('=') &&
        __index + 1 < execArgv.length &&
        !execArgv[__index + 1].startsWith('-')
      ) {
        __index += 1
      }
      continue
    }
    __workerExecArgv.push(__arg)
  }
  return __removed ? __workerExecArgv : undefined
}

function __createWasiWorker(filename) {
  let __workerExecArgv = __getWasiWorkerExecArgv()
  while (true) {
    try {
      return new Worker(filename, {
        env: process.env,
        execArgv: __workerExecArgv,
      })
    } catch (error) {
      if (!error || error.code !== 'ERR_WORKER_INVALID_EXEC_ARGV') {
        throw error
      }
      const __nextWorkerExecArgv =
        __removeInvalidWasiWorkerExecArgv(__workerExecArgv, error)
      if (!__nextWorkerExecArgv) {
        throw error
      }
      __workerExecArgv = __nextWorkerExecArgv
    }
  }
}

`;
  source = replaceExactly(
    source,
    WASI_NODE_HELPER_ANCHOR,
    workerHelpers + WASI_NODE_HELPER_ANCHOR,
    1,
    'WASI worker execArgv helpers',
  );
  return replaceExactly(
    source,
    WASI_NODE_WORKER_CONSTRUCTION,
    `    const worker = __createWasiWorker(__nodePath.join(__dirname, 'wasi-worker.mjs'))`,
    1,
    'WASI worker construction',
  );
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
    '  asyncWorkPoolSize: __rolldownAsyncWorkPoolSize,',
    1,
    'WASI async-work-pool option',
  );
  return replaceExactly(
    source,
    WASI_NODE_ENV_ASSIGNMENT,
    'env: __rolldownWasiEnv,',
    2,
    'WASI runtime and worker environments',
  );
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
