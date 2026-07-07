// @ts-nocheck This focused unit test mocks incompatible generated binding surfaces.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  loadedTarget: 'wasi-threads',
  report: {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'wasi-threads',
    threads: true,
    timers: true,
    wasi: true,
    watchSupported: false,
  } as Record<string, unknown>,
}));

vi.mock('../src/binding.cjs', () => ({
  get __rolldownBindingTarget() {
    return binding.loadedTarget;
  },
  getRuntimeCapabilities: () => binding.report,
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { getRuntimeCapabilitiesCompat } from '../src/runtime-support';

beforeEach(() => {
  binding.loadedTarget = 'wasi-threads';
  binding.report = {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'wasi-threads',
    threads: true,
    timers: true,
    wasi: true,
    watchSupported: false,
  };
});

test('capability reports must include every non-legacy field', () => {
  delete binding.report.wasi;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('wasi must be a boolean'),
    }),
  );
});

test('capability reports cannot contradict generated loader target metadata', () => {
  Object.assign(binding.report, {
    target: 'native',
    wasi: false,
    watchSupported: true,
  });

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('does not match the generated loader target'),
    }),
  );
});

test('thread and flavor contradictions fail closed', () => {
  binding.report.threads = false;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('threads does not agree'),
    }),
  );
});
