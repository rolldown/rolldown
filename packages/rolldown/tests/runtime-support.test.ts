import { rolldown } from 'rolldown';
import { getRuntimeSupport } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

// The `isWasiTest` / `isSingleThread` skips in tests/src/runtime-flavor.ts derive
// from this report, so a wrong one shifts pass/skip counts.

describe('getRuntimeSupport', () => {
  test('reports complete public workflow support', () => {
    const support = getRuntimeSupport();
    expect(Object.keys(support).sort()).toEqual([
      'dev',
      'parallelPlugins',
      'threadlessWasi',
      'watch',
      'workerd',
    ]);
    expect(Object.getOwnPropertyDescriptor(support, 'workerd')).toMatchObject({
      enumerable: true,
      value: false,
    });
    if (support.parallelPlugins) {
      // Native: every workflow.
      expect(support).toEqual({
        dev: true,
        watch: true,
        parallelPlugins: true,
        threadlessWasi: false,
        workerd: false,
      });
    } else if (support.threadlessWasi) {
      expect(support).toEqual({
        dev: false,
        watch: false,
        parallelPlugins: false,
        threadlessWasi: true,
        workerd: false,
      });
    } else {
      // Threaded WASI: `dev` follows the flavor, the rest is fixed.
      expect(support).toMatchObject({ watch: false, parallelPlugins: false, workerd: false });
    }
  });

  test('preserves structured plugin error metadata', async () => {
    const cause = Object.assign(new RangeError('nested plugin cause'), {
      nestedMarker: 17,
    });
    const original = Object.assign(new TypeError('plugin metadata failure'), {
      cause,
      code: 'USER_PLUGIN_CODE',
      customMarker: 'retained',
    });
    const bundle = await rolldown({
      input: 'entry',
      plugins: [
        {
          name: 'runtime-metadata-probe',
          resolveId(id) {
            if (id === 'entry') return '\0entry';
          },
          load(id) {
            if (id === '\0entry') return 'export default 1';
          },
          transform(_code, id) {
            if (id === '\0entry') throw original;
          },
        },
      ],
    });

    try {
      const failure = await bundle.generate().catch((error: unknown) => error);
      const [pluginError] = (failure as { errors?: unknown[] }).errors ?? [];
      expect(pluginError).toBe(original);
      expect(pluginError).toMatchObject({
        code: 'PLUGIN_ERROR',
        pluginCode: 'USER_PLUGIN_CODE',
        plugin: 'runtime-metadata-probe',
        hook: 'transform',
        id: '\0entry',
        customMarker: 'retained',
      });
      expect(original.stack).toContain('plugin metadata failure');
      expect(original.cause).toBe(cause);
      expect(original.cause).toMatchObject({
        name: 'RangeError',
        message: 'nested plugin cause',
        nestedMarker: 17,
      });
    } finally {
      await bundle.close();
    }
  });
});
