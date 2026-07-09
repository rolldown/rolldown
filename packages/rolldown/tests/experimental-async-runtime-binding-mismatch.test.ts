// @ts-nocheck This focused unit test mocks incompatible generated binding surfaces.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  configureAsyncRuntime: undefined as unknown,
  getAsyncRuntimeConfig: undefined as unknown,
  getAsyncRuntimeMetrics: undefined as unknown,
  resetAsyncRuntimeMetrics: undefined as unknown,
}));

vi.mock('../src/binding.cjs', () => ({
  get configureAsyncRuntime() {
    return binding.configureAsyncRuntime;
  },
  get getAsyncRuntimeConfig() {
    return binding.getAsyncRuntimeConfig;
  },
  get getAsyncRuntimeMetrics() {
    return binding.getAsyncRuntimeMetrics;
  },
  get resetAsyncRuntimeMetrics() {
    return binding.resetAsyncRuntimeMetrics;
  },
}));

const asyncRuntimeExports = [
  {
    name: 'configureAsyncRuntime',
    invoke: (api) => api.configureAsyncRuntime({}),
  },
  {
    name: 'getAsyncRuntimeConfig',
    invoke: (api) => api.getAsyncRuntimeConfig(),
  },
  {
    name: 'getAsyncRuntimeMetrics',
    invoke: (api) => api.getAsyncRuntimeMetrics(),
  },
  {
    name: 'resetAsyncRuntimeMetrics',
    invoke: (api) => api.resetAsyncRuntimeMetrics(),
  },
] as const;

beforeEach(() => {
  vi.resetModules();
  binding.configureAsyncRuntime = vi.fn();
  binding.getAsyncRuntimeConfig = vi.fn();
  binding.getAsyncRuntimeMetrics = vi.fn();
  binding.resetAsyncRuntimeMetrics = vi.fn();
});

test.each(asyncRuntimeExports)(
  'rejects a missing $name binding export',
  async ({ name, invoke }) => {
    binding[name] = undefined;
    // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
    const api = await import('../src/api/async-runtime');

    expect(() => invoke(api)).toThrow(
      expect.objectContaining({
        code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
        message: expect.stringContaining(`${name}() as a function`),
      }),
    );
  },
);

test.each(asyncRuntimeExports)(
  'rejects a malformed $name binding export',
  async ({ name, invoke }) => {
    binding[name] = {};
    // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
    const api = await import('../src/api/async-runtime');

    expect(() => invoke(api)).toThrow(
      expect.objectContaining({
        code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
        message: expect.stringContaining(`${name}() as a function`),
      }),
    );
  },
);
