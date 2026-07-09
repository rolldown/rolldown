export const LOADED_BINDING_TARGET_EXPORT = '__rolldownBindingTarget';
export const EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT = 4;
export const EMNAPI_ASYNC_WORK_POOL_SIZE_MAX = 1024;
export const ASYNC_RUNTIME_HOST_EXPORTS = [
  'getCurrentThreadTaskHostContractVersion',
  'registerCurrentThreadTaskHost',
  'registerTimerHost',
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
const WASI_NODE_HELPER_ANCHOR = 'const __rootDir = __nodePath.parse(process.cwd()).root\n';
const WASI_NODE_ENV_ASSIGNMENT = 'env: process.env,';
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
  const count = source.split(search).length - 1;
  if (count !== expectedCount) {
    throw new Error(
      `Unexpected NAPI-RS loader template for ${label}: expected ${expectedCount} anchors, found ${count}`,
    );
  }
  return source.replaceAll(search, replacement);
}
