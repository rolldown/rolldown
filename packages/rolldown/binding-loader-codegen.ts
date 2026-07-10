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
const WASI_NODE_CONTEXT_CREATION =
  '  __emnapiContext = __emnapiCreateContext({ autoDestroy: false })\n';
const WASI_NODE_CONTEXT_SUPPRESS_DESTROY = '  __emnapiContext.suppressDestroy()\n';
const WASI_NAPI_INSTANCE_DECLARATION = 'let __napiInstance\n';
const WASI_NAPI_INSTANCE_ASSIGNMENT = '      __napiInstance = instance\n';
const WASI_CONTEXT_DESTROY_HELPER = 'function __destroyEmnapiContext() {\n';
const WASI_CONTEXT_PREPARE_CLEANUP = `  const __prepareWasmEnvCleanup =
    __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
  if (typeof __prepareWasmEnvCleanup === 'function') {
    __prepareWasmEnvCleanup()
  }
`;
const WASI_CONTEXT_PREPARE_CLEANUP_GUARD = `  if (!__emnapiWasmEnvCleanupPrepared) {
    const __prepareWasmEnvCleanup =
      __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
    if (typeof __prepareWasmEnvCleanup === 'function') {
      __prepareWasmEnvCleanup()
    }
    __emnapiWasmEnvCleanupPrepared = true
  }
`;
const WASI_NODE_CONTEXT_PREPARE_CLEANUP = `    const __prepareWasmEnvCleanup =
      __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
    if (typeof __prepareWasmEnvCleanup === 'function') {
      __prepareWasmEnvCleanup()
    }
`;
const WASI_NODE_CONTEXT_PREPARE_CLEANUP_GUARD = `    if (!__emnapiWasmEnvCleanupPrepared) {
      const __prepareWasmEnvCleanup =
        __napiInstance?.exports?.napi_prepare_wasm_env_cleanup
      if (typeof __prepareWasmEnvCleanup === 'function') {
        __prepareWasmEnvCleanup()
      }
      __emnapiWasmEnvCleanupPrepared = true
    }
`;
const WASI_BEFORE_INIT_ANCHOR = '  beforeInit({ instance }) {\n';
const WASI_CONTEXT_DESTROY_CALL = '    __wrapEmnapiContextDestroy(instance)\n';
const WASI_NODE_HELPER_ANCHOR = 'const __rootDir = __nodePath.parse(process.cwd()).root\n';
const WASI_NODE_ENV_ASSIGNMENT = 'env: process.env,';
const WASI_NODE_WORKER_CONSTRUCTION = `    const worker = new Worker(__nodePath.join(__dirname, 'wasi-worker.mjs'), {
      env: process.env,
    })`;
const WASI_BROWSER_SYNC_WORKER_TERMINATION =
  '      __terminations.push({ error: __cleanupError })\n';
const WASI_BROWSER_ASYNC_WORKER_TERMINATION =
  '      __terminations.push(Promise.resolve({ error: __cleanupError }))\n';
const WASI_NODE_UPSTREAM_WORKER_HELPERS = `function __getWorkerExecArgv() {
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

function __createWasiWorker(filename) {
  try {
    return new Worker(filename, {
      env: process.env,
      execArgv: __getWorkerExecArgv(),
    })
  } catch (error) {
    if (!error || error.code !== 'ERR_WORKER_INVALID_EXEC_ARGV') {
      throw error
    }
  }
  return new Worker(filename, {
    env: process.env,
    execArgv: [],
  })
}

`;
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

  const generatedDestroyHelperCount = countOccurrences(source, WASI_CONTEXT_DESTROY_HELPER);
  if (generatedDestroyHelperCount > 0) {
    if (generatedDestroyHelperCount !== 1) {
      throw new Error(
        `Unexpected NAPI-RS WASI loader template for context destroy helper: expected one helper, found ${generatedDestroyHelperCount}`,
      );
    }
    if (source.includes('function __wrapEmnapiContextDestroy(instance)')) {
      throw new Error('Unexpected NAPI-RS WASI loader template: mixed context lifecycle helpers');
    }

    const browserContextCount = countOccurrences(source, WASI_ISOLATED_CONTEXT_CREATION);
    const nodeContextCount = countOccurrences(source, WASI_NODE_CONTEXT_CREATION);
    if (browserContextCount + nodeContextCount !== 1) {
      throw new Error(
        `Unexpected NAPI-RS WASI loader template for context creation: expected one browser or Node lifecycle, found ${browserContextCount + nodeContextCount}`,
      );
    }
    assertExactlyOne(source, WASI_NAPI_INSTANCE_DECLARATION, 'WASI N-API instance declaration');
    assertExactlyOne(source, WASI_NAPI_INSTANCE_ASSIGNMENT, 'WASI N-API instance capture');
    assertExactlyOne(source, '__emnapiContext.destroy()', 'WASI context destroy operation');
    if (nodeContextCount === 1) {
      assertExactlyOne(
        source,
        WASI_NODE_CONTEXT_SUPPRESS_DESTROY,
        'WASI Node context auto-destroy suppression',
      );
      for (const signature of [
        'function __destroyEmnapiContextBeforeExit() {',
        'function __destroyEmnapiContextAtExit() {',
        'function __handoffEmnapiContextCleanupToExit() {',
      ]) {
        assertExactlyOne(source, signature, 'WASI Node context lifecycle helper');
      }
    }

    const preparation =
      nodeContextCount === 1 ? WASI_NODE_CONTEXT_PREPARE_CLEANUP : WASI_CONTEXT_PREPARE_CLEANUP;
    const guardedPreparation =
      nodeContextCount === 1
        ? WASI_NODE_CONTEXT_PREPARE_CLEANUP_GUARD
        : WASI_CONTEXT_PREPARE_CLEANUP_GUARD;
    const preparationCount = countOccurrences(source, preparation);
    const guardedPreparationCount = countOccurrences(source, guardedPreparation);
    const preparationFlagCount = countOccurrences(
      source,
      'let __emnapiWasmEnvCleanupPrepared = false\n',
    );
    if (preparationCount === 0 && guardedPreparationCount === 1 && preparationFlagCount === 1) {
      return source;
    }
    if (preparationCount !== 1 || guardedPreparationCount !== 0 || preparationFlagCount !== 0) {
      throw new Error('Unexpected NAPI-RS WASI loader template for context cleanup preparation');
    }

    source = replaceExactly(
      source,
      WASI_CONTEXT_DESTROY_HELPER,
      `let __emnapiWasmEnvCleanupPrepared = false

${WASI_CONTEXT_DESTROY_HELPER}`,
      1,
      'WASI context cleanup preparation state',
    );
    return replaceExactly(
      source,
      preparation,
      guardedPreparation,
      1,
      'WASI context cleanup preparation',
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
  const rolldownHelperCount = countOccurrences(
    source,
    'function __removeInvalidWasiWorkerExecArgv(execArgv, error) {',
  );
  if (rolldownHelperCount > 0) {
    if (
      rolldownHelperCount !== 1 ||
      countOccurrences(source, 'function __getWasiWorkerExecArgv() {') !== 1 ||
      countOccurrences(source, 'function __createWasiWorker(filename) {') !== 1
    ) {
      throw new Error('Unexpected Rolldown WASI worker execArgv helper template');
    }
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
  const upstreamWorkerHelperCount = countOccurrences(source, 'function __getWorkerExecArgv() {');
  const workerFactoryCount = countOccurrences(source, 'function __createWasiWorker(filename) {');
  if (upstreamWorkerHelperCount > 0 || workerFactoryCount > 0) {
    if (upstreamWorkerHelperCount !== 1 || workerFactoryCount !== 1) {
      throw new Error('Unexpected NAPI-RS WASI worker execArgv helper template');
    }
    return replaceExactly(
      source,
      WASI_NODE_UPSTREAM_WORKER_HELPERS,
      workerHelpers,
      1,
      'WASI worker execArgv helpers',
    );
  }

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
  const environmentCount = countOccurrences(source, WASI_NODE_ENV_ASSIGNMENT);
  if (environmentCount !== 2 && environmentCount !== 3) {
    throw new Error(
      `Unexpected NAPI-RS loader template for WASI runtime and worker environments: expected 2 or 3 anchors, found ${environmentCount}`,
    );
  }
  return source.replaceAll(WASI_NODE_ENV_ASSIGNMENT, 'env: __rolldownWasiEnv,');
}

export function patchWasiBrowserContextDestroyAwait(source: string): string {
  const expressions = ['__emnapiContext.destroy()', '__destroyEmnapiContext()'] as const;
  const matches = expressions.flatMap((expression) => {
    const generated = `await ${expression}`;
    const normalized = `await Promise.resolve(${expression})`;
    return [
      { count: countOccurrences(source, generated), expression, generated, normalized: false },
      {
        count: countOccurrences(source, normalized),
        expression,
        generated: normalized,
        normalized: true,
      },
    ];
  });
  const matched = matches.filter(({ count }) => count > 0);
  const total = matched.reduce((count, match) => count + match.count, 0);
  if (total !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI browser cleanup template: expected one context destroy await, found ${total}`,
    );
  }
  const [match] = matched;
  if (match.count !== 1) {
    throw new Error(
      `Unexpected NAPI-RS WASI browser cleanup template: expected one context destroy await, found ${match.count}`,
    );
  }
  if (match.normalized) {
    return source;
  }
  return source.replace(match.generated, `await Promise.resolve(${match.expression})`);
}

export function patchWasiBrowserWorkerTerminationAwait(source: string): string {
  const generatedCount = countOccurrences(source, WASI_BROWSER_SYNC_WORKER_TERMINATION);
  const normalizedCount = countOccurrences(source, WASI_BROWSER_ASYNC_WORKER_TERMINATION);
  if (generatedCount === 0 && normalizedCount === 1) {
    return source;
  }
  if (generatedCount !== 1 || normalizedCount !== 0) {
    throw new Error(
      `Unexpected NAPI-RS WASI browser worker cleanup template: expected one synchronous termination result, found ${generatedCount}`,
    );
  }
  return source.replace(
    WASI_BROWSER_SYNC_WORKER_TERMINATION,
    WASI_BROWSER_ASYNC_WORKER_TERMINATION,
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

function assertExactlyOne(source: string, search: string, label: string): void {
  const count = countOccurrences(source, search);
  if (count !== 1) {
    throw new Error(
      `Unexpected NAPI-RS loader template for ${label}: expected 1 anchor, found ${count}`,
    );
  }
}
