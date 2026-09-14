import { expect, test, vi } from 'vitest';

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => ({
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
    wasi: false,
    watchSupported: true,
  }),
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { UnsupportedRuntimeFeatureError } from '../src/runtime-support';

test('unsupported-feature errors remain coherent when constructed for an available feature', () => {
  const error = new UnsupportedRuntimeFeatureError('pluginErrorMetadata');

  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'pluginErrorMetadata',
    runtime: {
      flavor: 'MultiThread',
      target: 'native',
    },
  });
  expect(error.message).toBe(
    "structured plugin error metadata is supported by Rolldown's MultiThread runtime on the native target. " +
      'UnsupportedRuntimeFeatureError was constructed for an available feature.',
  );
  expect(error.message).not.toContain('not supported');
});
