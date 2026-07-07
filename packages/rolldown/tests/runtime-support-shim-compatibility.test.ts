// @ts-nocheck This focused unit test mocks an incomplete development binding shim.
import { expect, test, vi } from 'vitest';

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: undefined,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { getRuntimeCapabilitiesCompat, getRuntimeSupport } from '../src/runtime-support';

test('bindings without a capability reporter preserve legacy native feature support', () => {
  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    devSupported: true,
    target: 'native',
    watchSupported: true,
  });
  expect(getRuntimeSupport()).toEqual({
    dev: true,
    parallelPlugins: true,
    viteDynamicImportVarsResolver: true,
    watch: true,
  });
});
