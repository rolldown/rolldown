import { describe, expect, test } from 'vitest';

import {
  assertAsyncRuntimeHostExports,
  ASYNC_RUNTIME_HOST_EXPORTS,
  EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT,
  EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
  LOADED_BINDING_TARGET_EXPORT,
  normalizeEmnapiAsyncWorkPoolSize,
  patchWasiBindingLoader,
  patchWasiNodeAsyncWorkPoolSize,
  resolveWasiBindingTarget,
} from '../binding-loader-codegen';

const cjsAnchor = 'module.exports = __napiModule.exports\n';
const esmAnchor = 'export default __napiModule.exports\n';
const wasiNodeLoaderTemplate = `const __nodePath = { parse: () => ({ root: '/' }) }
const __rootDir = __nodePath.parse(process.cwd()).root
const __wasiOptions = {
  env: process.env,
}
const __emnapiOptions = {
  asyncWorkPoolSize: (function() {
    const threadsSizeFromEnv = Number(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE ?? process.env.UV_THREADPOOL_SIZE)
    // NaN > 0 is false
    if (threadsSizeFromEnv > 0) {
      return threadsSizeFromEnv
    } else {
      return 4
    }
  })(),
}
const __workerOptions = {
  env: process.env,
}
`;

describe('WASI binding target metadata', () => {
  test('resolves supported build targets without accepting unknown wasm targets', () => {
    expect(resolveWasiBindingTarget(undefined)).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('aarch64-apple-darwin')).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('wasm32-wasip1-threads')).toBe('wasi-threads');
    expect(resolveWasiBindingTarget('wasm32-wasip1')).toBe('wasi');
    expect(() => resolveWasiBindingTarget('wasm32-wasip2')).toThrow(
      'Unsupported WASI binding target',
    );
    expect(() => resolveWasiBindingTarget(null)).toThrow('Unsupported WASI binding target');
  });

  test.each([
    ['CommonJS', cjsAnchor, `module.exports.${LOADED_BINDING_TARGET_EXPORT}`],
    ['ESM', esmAnchor, `export const ${LOADED_BINDING_TARGET_EXPORT}`],
  ])('replaces %s metadata across repeated and reversed builds', (_name, anchor, exportName) => {
    const threaded = patchWasiBindingLoader(anchor, 'wasi-threads');
    expect(threaded).toContain(`${exportName} = 'wasi-threads'`);

    const threadless = patchWasiBindingLoader(threaded, 'wasi');
    expect(threadless).toContain(`${exportName} = 'wasi'`);
    expect(threadless).not.toContain(`${exportName} = 'wasi-threads'`);

    const reversed = patchWasiBindingLoader(threadless, 'wasi-threads');
    expect(reversed).toContain(`${exportName} = 'wasi-threads'`);
    expect(reversed).not.toContain(`${exportName} = 'wasi'`);
    expect(patchWasiBindingLoader(reversed, 'wasi-threads')).toBe(reversed);
  });

  test('rejects duplicate target exports instead of preserving the stale winner', () => {
    const duplicate = `${cjsAnchor}module.exports.${LOADED_BINDING_TARGET_EXPORT} = 'wasi'\nmodule.exports.${LOADED_BINDING_TARGET_EXPORT} = 'wasi-threads'\n`;
    expect(() => patchWasiBindingLoader(duplicate, 'wasi')).toThrow(
      'expected at most one binding target export',
    );
  });

  test.each([
    [
      'CommonJS',
      `${cjsAnchor}module.exports.${LOADED_BINDING_TARGET_EXPORT} = "unknown";\n`,
      `module.exports.${LOADED_BINDING_TARGET_EXPORT}`,
    ],
    [
      'ESM',
      `${esmAnchor}export const ${LOADED_BINDING_TARGET_EXPORT} = "unknown";\n`,
      `export const ${LOADED_BINDING_TARGET_EXPORT}`,
    ],
  ])(
    'replaces an unexpected existing %s target without adding a duplicate',
    (_name, source, exportName) => {
      const patched = patchWasiBindingLoader(source, 'wasi');
      expect(patched).toContain(`${exportName} = 'wasi'`);
      expect(patched.match(new RegExp(exportName.replaceAll('.', '\\.'), 'g'))).toHaveLength(1);
      expect(patched).not.toContain('unknown');
    },
  );
});

describe('WASI async work pool normalization', () => {
  test.each([
    [undefined, EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['0', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['0.5', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['invalid', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['Infinity', EMNAPI_ASYNC_WORK_POOL_SIZE_DEFAULT],
    ['1.9', 1],
    ['1e2', 100],
    ['0x10', 16],
    ['2048', EMNAPI_ASYNC_WORK_POOL_SIZE_MAX],
  ])('normalizes %j to %d', (value, expected) => {
    expect(normalizeEmnapiAsyncWorkPoolSize(value)).toBe(expected);
  });

  test('the generated Node loader gives emnapi and the WASI guest the same capped value', () => {
    const patched = patchWasiNodeAsyncWorkPoolSize(wasiNodeLoaderTemplate);
    const process = {
      cwd: () => '/',
      env: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: '2048',
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
    };
    // oxlint-disable-next-line typescript/no-implied-eval -- evaluate the generated loader snippet in an isolated scope
    const result = Function(
      'process',
      `${patched}
return {
  pool: __emnapiOptions.asyncWorkPoolSize,
  wasiEnv: __wasiOptions.env,
  workerEnv: __workerOptions.env,
}`,
    )(process);

    expect(result).toEqual({
      pool: EMNAPI_ASYNC_WORK_POOL_SIZE_MAX,
      wasiEnv: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: String(EMNAPI_ASYNC_WORK_POOL_SIZE_MAX),
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
      workerEnv: {
        NAPI_RS_ASYNC_WORK_POOL_SIZE: String(EMNAPI_ASYNC_WORK_POOL_SIZE_MAX),
        UV_THREADPOOL_SIZE: '2',
        UNRELATED: 'preserved',
      },
    });
    expect(process.env.NAPI_RS_ASYNC_WORK_POOL_SIZE).toBe('2048');
    expect(patchWasiNodeAsyncWorkPoolSize(patched)).toBe(patched);
  });

  test('the generated Node loader normalizes the UV fallback into the authoritative NAPI key', () => {
    const patched = patchWasiNodeAsyncWorkPoolSize(wasiNodeLoaderTemplate);
    // oxlint-disable-next-line typescript/no-implied-eval -- evaluate the generated loader snippet in an isolated scope
    const result = Function(
      'process',
      `${patched}
return {
  pool: __emnapiOptions.asyncWorkPoolSize,
  wasiEnv: __wasiOptions.env,
}`,
    )({
      cwd: () => '/',
      env: { UV_THREADPOOL_SIZE: '6' },
    });

    expect(result.pool).toBe(6);
    expect(result.wasiEnv).toEqual({
      NAPI_RS_ASYNC_WORK_POOL_SIZE: '6',
      UV_THREADPOOL_SIZE: '6',
    });
  });
});

describe('async-runtime host export contract', () => {
  test.each([
    [
      'CommonJS',
      'commonjs' as const,
      ASYNC_RUNTIME_HOST_EXPORTS.map(
        (name) => `module.exports.${name} = nativeBinding.${name}\n`,
      ).join(''),
    ],
    [
      'ESM',
      'esm' as const,
      ASYNC_RUNTIME_HOST_EXPORTS.map(
        (name) => `export const ${name} = __napiModule.exports.${name}\n`,
      ).join(''),
    ],
  ])('accepts a complete generated %s loader', (_name, format, source) => {
    expect(() => assertAsyncRuntimeHostExports(source, format)).not.toThrow();
  });

  test('reports every missing named export', () => {
    expect(() =>
      assertAsyncRuntimeHostExports(
        'module.exports.registerTimerHost = nativeBinding.registerTimerHost\n',
        'commonjs',
      ),
    ).toThrow(
      'getCurrentThreadTaskHostContractVersion, registerCurrentThreadTaskHost, unregisterCurrentThreadTaskHost, unregisterTimerHost',
    );
  });
});
