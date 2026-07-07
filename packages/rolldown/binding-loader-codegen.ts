export const LOADED_BINDING_TARGET_EXPORT = '__rolldownBindingTarget';

type LoadedBindingTarget = 'native' | 'wasi' | 'wasi-threads';
export type WasiBindingTarget = Exclude<LoadedBindingTarget, 'native'>;

const WASI_TARGET = 'wasm32-wasip1';
const WASI_THREADS_TARGET = 'wasm32-wasip1-threads';
const NATIVE_BINDING_ANCHOR = 'let nativeBinding = null\n';
const WASI_BINDING_ASSIGNMENT = 'nativeBinding = wasiBinding';
const NATIVE_BINDING_EXPORT_ANCHOR = 'module.exports = nativeBinding\n';
const WASI_CJS_EXPORT_ANCHOR = 'module.exports = __napiModule.exports\n';
const WASI_ESM_EXPORT_ANCHOR = 'export default __napiModule.exports\n';
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
