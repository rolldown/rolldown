import { describe, expect, test } from 'vitest';

import {
  LOADED_BINDING_TARGET_EXPORT,
  patchWasiBindingLoader,
  resolveWasiBindingTarget,
} from '../binding-loader-codegen';

const cjsAnchor = 'module.exports = __napiModule.exports\n';
const esmAnchor = 'export default __napiModule.exports\n';

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
