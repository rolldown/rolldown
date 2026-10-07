import { expect, test, vi } from 'vitest';

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => ({
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    wasi: false,
    watchSupported: true,
  }),
}));

import { UnsupportedRuntimeFeatureError } from '../src/runtime-support';

test('unsupported-feature errors remain coherent when constructed for an available feature', () => {
  const error = new UnsupportedRuntimeFeatureError('dev');

  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'dev',
    runtime: {
      flavor: 'MultiThread',
      target: 'native',
    },
  });
  expect(error.message).toBe(
    "dev() is supported by Rolldown's MultiThread runtime on the native target. " +
      'UnsupportedRuntimeFeatureError was constructed for an available feature.',
  );
  expect(error.message).not.toContain('not supported');
});
