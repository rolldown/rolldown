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
} from '../src/runtime-support';

test('older capability reports derive dev support from scheduler threads', () => {
  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    devSupported: true,
    watchSupported: true,
  });
  expect(getRuntimeSupport()).toEqual({
    dev: true,
    parallelPlugins: true,
    viteDynamicImportVarsResolver: true,
    watch: true,
  });
  expect(() => assertRuntimeFeature('dev')).not.toThrow();
  expect(() => assertRuntimeFeature('watch')).not.toThrow();
  expect(() => assertRuntimeFeature('parallelPlugins')).not.toThrow();
});
