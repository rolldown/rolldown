// @ts-nocheck This focused unit test mocks incompatible generated binding surfaces.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  configureAsyncRuntime: undefined as unknown,
  exportErrors: new Map<string, unknown>(),
  getAsyncRuntimeConfig: undefined as unknown,
  getAsyncRuntimeMetrics: undefined as unknown,
  resetAsyncRuntimeMetrics: undefined as unknown,
}));
const validConfig = {
  flavor: 'MultiThread',
  maxBlockingTasks: 1,
  workerThreads: 2,
};
const validMetrics = {
  ...validConfig,
  activeBlockingTasks: 0,
  activeRunnables: 0,
  blockingTasksCompleted: 0,
  blockingTasksStarted: 0,
  maxActiveBlockingTasks: 0,
  maxActiveRunnables: 0,
  maxQueuedRunnables: 0,
  queuedRunnables: 0,
  runnablePolls: 0,
  runnableSchedules: 0,
  tasksCompleted: 0,
  tasksPanicked: 0,
  tasksSpawned: 0,
};

vi.mock('../src/binding.cjs', () => ({
  get configureAsyncRuntime() {
    if (binding.exportErrors.has('configureAsyncRuntime')) {
      throw binding.exportErrors.get('configureAsyncRuntime');
    }
    return binding.configureAsyncRuntime;
  },
  get getAsyncRuntimeConfig() {
    if (binding.exportErrors.has('getAsyncRuntimeConfig')) {
      throw binding.exportErrors.get('getAsyncRuntimeConfig');
    }
    return binding.getAsyncRuntimeConfig;
  },
  get getAsyncRuntimeMetrics() {
    if (binding.exportErrors.has('getAsyncRuntimeMetrics')) {
      throw binding.exportErrors.get('getAsyncRuntimeMetrics');
    }
    return binding.getAsyncRuntimeMetrics;
  },
  get resetAsyncRuntimeMetrics() {
    if (binding.exportErrors.has('resetAsyncRuntimeMetrics')) {
      throw binding.exportErrors.get('resetAsyncRuntimeMetrics');
    }
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
  binding.exportErrors.clear();
  binding.getAsyncRuntimeConfig = vi.fn(() => validConfig);
  binding.getAsyncRuntimeMetrics = vi.fn(() => validMetrics);
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
  'wraps a throwing $name export getter as a binding mismatch',
  async ({ name, invoke }) => {
    const cause = new Error(`${name} getter failed`);
    binding.exportErrors.set(name, cause);
    // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
    const api = await import('../src/api/async-runtime');

    expect(() => invoke(api)).toThrow(
      expect.objectContaining({
        cause,
        code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
        message: expect.stringContaining('the export could not be read'),
      }),
    );
  },
);

test.each([
  ['getAsyncRuntimeConfig', (api) => api.getAsyncRuntimeConfig()],
  ['getAsyncRuntimeMetrics', (api) => api.getAsyncRuntimeMetrics()],
])('wraps a throwing %s reporter as a binding mismatch', async (name, invoke) => {
  const cause = new Error(`${name} reporter failed`);
  binding[name] = vi.fn(() => {
    throw cause;
  });
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  expect(() => invoke(api)).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('the reporter threw'),
    }),
  );
});

test('an undefined reporter failure remains available as the mismatch cause', async () => {
  binding.getAsyncRuntimeConfig = vi.fn(() => {
    throw undefined;
  });
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  let error: unknown;
  try {
    api.getAsyncRuntimeConfig();
  } catch (caught) {
    error = caught;
  }

  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('the reporter threw'),
  });
  expect(Object.prototype.hasOwnProperty.call(error, 'cause')).toBe(true);
  expect((error as Error).cause).toBeUndefined();
});

test.each([
  ['a non-object result', null],
  ['an unknown flavor', { ...validConfig, flavor: 'ThreadPool' }],
  ['a zero worker count', { ...validConfig, workerThreads: 0 }],
  ['a fractional blocking limit', { ...validConfig, maxBlockingTasks: 1.5 }],
  [
    'a CurrentThread report with multiple workers',
    { flavor: 'CurrentThread', workerThreads: 2, maxBlockingTasks: 1 },
  ],
  [
    'a CurrentThread report with multiple blocking tasks',
    { flavor: 'CurrentThread', workerThreads: 1, maxBlockingTasks: 2 },
  ],
])('rejects %s from getAsyncRuntimeConfig', async (_name, result) => {
  binding.getAsyncRuntimeConfig = vi.fn(() => result);
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  expect(() => api.getAsyncRuntimeConfig()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('incompatible getAsyncRuntimeConfig() result'),
    }),
  );
});

test('preserves a throwing config field getter as the mismatch cause', async () => {
  const cause = new Error('workerThreads getter failed');
  binding.getAsyncRuntimeConfig = vi.fn(() => ({
    ...validConfig,
    get workerThreads() {
      throw cause;
    },
  }));
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  expect(() => api.getAsyncRuntimeConfig()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('workerThreads field could not be read'),
    }),
  );
});

test.each([
  ['a negative counter', { ...validMetrics, tasksSpawned: -1 }],
  [
    'a CurrentThread report with multiple blocking tasks',
    {
      ...validMetrics,
      flavor: 'CurrentThread',
      maxBlockingTasks: 2,
      workerThreads: 1,
    },
  ],
  [
    'a maximum below its live gauge',
    { ...validMetrics, maxActiveRunnables: 1, activeRunnables: 2 },
  ],
])('rejects %s from getAsyncRuntimeMetrics', async (_name, result) => {
  binding.getAsyncRuntimeMetrics = vi.fn(() => result);
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  expect(() => api.getAsyncRuntimeMetrics()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('incompatible getAsyncRuntimeMetrics() result'),
    }),
  );
});

test('preserves a throwing metrics field getter as the mismatch cause', async () => {
  const cause = new Error('tasksCompleted getter failed');
  binding.getAsyncRuntimeMetrics = vi.fn(() => ({
    ...validMetrics,
    get tasksCompleted() {
      throw cause;
    },
  }));
  // @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
  const api = await import('../src/api/async-runtime');

  expect(() => api.getAsyncRuntimeMetrics()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('tasksCompleted field could not be read'),
    }),
  );
});

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
