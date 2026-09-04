// @ts-nocheck This focused unit test mocks an older generated binding surface.
import { expect, test, vi } from 'vitest';

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => ({
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
    wasi: false,
    watchSupported: true,
  }),
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import {
  assertRuntimeFeature,
  getRuntimeCapabilitiesCompat,
  getRuntimeSupport,
  UnsupportedRuntimeFeatureError,
} from '../src/runtime-support';

test('older capability reports derive dev support from scheduler threads', () => {
  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    devSupported: true,
    watchSupported: true,
  });
  expect(getRuntimeSupport()).toEqual({
    dev: true,
    dynamicImportVarsResolver: true,
    importGlobResolver: true,
    parallelPlugins: true,
    pluginErrorMetadata: true,
    symlinks: true,
    threadlessWasi: false,
    watch: true,
    workerd: false,
  });
  expect(() => assertRuntimeFeature('dev')).not.toThrow();
  expect(() => assertRuntimeFeature('watch')).not.toThrow();
  expect(() => assertRuntimeFeature('parallelPlugins')).not.toThrow();
});

test('unsupported-feature errors remain coherent when constructed for an available feature', () => {
  const error = new UnsupportedRuntimeFeatureError('pluginErrorMetadata', {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: false,
    flavor: 'CurrentThread',
    target: 'wasi',
    threads: false,
    timers: true,
    wasi: true,
    watchSupported: false,
  });

  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'pluginErrorMetadata',
    runtime: {
      flavor: 'CurrentThread',
      target: 'wasi',
    },
  });
  expect(error.message).toBe(
    "structured plugin error metadata is supported by Rolldown's CurrentThread runtime on the wasi target. " +
      'UnsupportedRuntimeFeatureError was constructed for an available feature.',
  );
  expect(error.message).not.toContain('not supported');
});
