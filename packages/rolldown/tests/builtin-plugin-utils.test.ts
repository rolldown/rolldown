// @ts-nocheck This focused unit test mocks the generated binding surface.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  class BindingCallableBuiltinPlugin {
    getOrder() {
      return null;
    }
  }

  return {
    BindingCallableBuiltinPlugin,
    getRuntimeCapabilities: () => ({
      asyncRuntimeBuild: true,
      backend: 'shared',
      blockOnJsThreadSafe: false,
      devSupported: true,
      flavor: 'MultiThread',
      target: 'wasi-threads',
      threads: true,
      timers: true,
      wasi: true,
      watchSupported: false,
    }),
  };
});

vi.mock('../src/binding.cjs', () => binding);

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { bindingifyManifestPlugin, BuiltinPlugin } from '../src/builtin-plugin/utils';

beforeEach(() => {
  vi.clearAllMocks();
});

test('manifest legacy callback retains the supplied options as its receiver', () => {
  const pluginOptions = {
    root: '/project',
    isOutputOptionsForLegacyChunks(outputOptions) {
      expect(this).toBe(pluginOptions);
      expect(outputOptions).toBe(normalizedOutputOptions);
      return true;
    },
  };
  const normalizedOutputOptions = {};
  const bindingOptions = bindingifyManifestPlugin(
    new BuiltinPlugin('builtin:vite-manifest', pluginOptions),
    {
      getOutputOptions: vi.fn(() => normalizedOutputOptions),
    },
  ).options;

  expect(bindingOptions.isLegacy({})).toBe(true);
});
